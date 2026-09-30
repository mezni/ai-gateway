# Data Model: Domain Model Validation

**Feature**: [spec.md](spec.md)

**Date**: 2026-09-30

This phase adds two generation controls to the provider-independent chat
request, one ordered validation rule set over the chat request, and one new
client-facing error row. No durable storage, provider entity, endpoint, or
client-visible success field is created.

The rule catalog itself — the authoritative statement of every condition and
response — lives in [contracts/validation-rules.md](contracts/validation-rules.md)
and is summarized here only where a field or constant is defined.

## Domain Entities

### Generation control ranges

| Constant | Value | Meaning |
|----------|-------|---------|
| `TEMPERATURE_MIN` | `0.0` | Lowest accepted sampling temperature, inclusive |
| `TEMPERATURE_MAX` | `2.0` | Highest accepted sampling temperature, inclusive |
| `MAX_TOKENS_MIN` | `1` | Lowest accepted response-length limit, inclusive |
| `MAX_TOKENS_MAX` | `4096` | Highest accepted response-length limit, inclusive |

Both bounds are inclusive. Values are gateway-level for this phase; per-model,
per-tenant, and policy-based limits belong to the routing and policy phases.

### ChatRequest

The provider-independent request that validation produces. It is the only request
form that reaches application logic.

| Attribute | Type | Rules |
|-----------|------|-------|
| model | String | Non-empty; preserved exactly, including surrounding whitespace; no registry lookup in this phase |
| messages | Vec\<Message\> | At least one message, preserved in submitted order |
| temperature | Option\<f64\> | `None` means the client did not specify it; `Some` carries the exact submitted value and is always within `TEMPERATURE_MIN..=TEMPERATURE_MAX` |
| max_tokens | Option\<u32\> | `None` means the client did not specify it; `Some` carries the exact submitted value and is always within `MAX_TOKENS_MIN..=MAX_TOKENS_MAX` |

The `Option` state is the only representation of "unspecified". No default value
is ever materialized, so a validated request can never present a number the
client did not send.

| Constructor | Behavior |
|-------------|----------|
| `ChatRequest::new(model, messages)` | Builds a request with both controls unspecified. Unchanged from Phase 1, so existing callers and assertions keep working. |
| `ChatRequest::with_controls(model, messages, temperature, max_tokens)` | Builds a request carrying the supplied control state. |

Because the validated request guarantees the ranges, no consumer needs to
re-check them, and no provider adapter can receive an out-of-range control.

### Message

Unchanged from Phase 1.

| Attribute | Type | Rules |
|-----------|------|-------|
| role | MessageRole | Exactly `system`, `user`, or `assistant` |
| content | String | Non-empty text; whitespace preserved; no trimming and no per-message size limit in this phase |

### MessageRole

Unchanged from Phase 1: `System` → `system`, `User` → `user`,
`Assistant` → `assistant`.

### ChatResponse and Usage

Unchanged from Phase 2. The mock continues to return `usage: None`; this phase
adds no token counting.

## Application Entities

### IncomingMessage

Unchanged: the transport-neutral message as supplied by the API, with `role` as
a raw string that the application converts.

### CompleteChatCommand

The inward-facing input created by the API after structural JSON parsing. Its
shape is extended with the same two controls as the domain request.

| Attribute | Type | Rules |
|-----------|------|-------|
| model | String | Validated as non-empty by the application |
| messages | Vec\<IncomingMessage\> | Validated as non-empty by the application |
| temperature | Option\<f64\> | Already strictly typed; the application only checks the range |
| max_tokens | Option\<u32\> | Already strictly typed; the application only checks the range |
| stream | bool | Carried so the ordered rule set can evaluate streaming last |

`stream` remains part of the command rather than a handler-local value so that
the whole precedence order — required fields, then control ranges, then
streaming — is evaluated in one place. The API does not branch on it.

### ValidatedChat

An opaque application value wrapping the validated domain request. Application
logic can only obtain it from `validate`, and `complete` consumes it by value.

| Accessor | Returns |
|----------|---------|
| `model()` | The validated model string |
| `temperature()` | `Option<f64>`, the validated control state |
| `max_tokens()` | `Option<u32>`, the validated control state |
| `messages()` | The validated, ordered message list |

The wrapper is what makes FR-014 structural rather than conventional: there is
no signature by which an unvalidated `CompleteChatCommand` can reach
`complete`.

### ChatCompletion

Unchanged: the transport-neutral application result with a single `content`
field. It carries no usage and no echo of the generation controls, because no
model is connected and the phase introduces no new client-visible success field.

### ValidationFailure

The application-layer classification of a rejected command. It is deliberately
coarse: every field-level failure collapses to the single public
`invalid_request` row, and the offending field is recorded only to make the
ordered rule set testable — it is never serialized to a client.

| Variant | Raised by | Public response |
|---------|-----------|-----------------|
| `InvalidModel` | Empty `model` | 400 `invalid_request` |
| `InvalidMessages` | Empty message list | 400 `invalid_request` |
| `InvalidRole` | Unsupported role in any position | 400 `invalid_request` |
| `InvalidContent` | Empty `content` in any position | 400 `invalid_request` |
| `TemperatureOutOfRange` | Present but outside `0.0..=2.0` | 400 `invalid_request` |
| `MaxTokensOutOfRange` | Present but outside `1..=4096` | 400 `invalid_request` |
| `StreamingRequested` | `stream == true` | 400 `unsupported_feature` |

Type-level failures — a control that is a string, boolean, `null`, object, or
array, a control supplied twice, a message that is not an object — never reach
this entity. They fail during deserialization and are already normalized to
`invalid_request` by the existing `JsonRejection` mapping.

The error type keeps the fixed public message `The chat request is invalid.`
(streams: `Streaming is not supported.`) and no field, value, or source detail,
so a `Display` of it can never leak input.

### MockChatCompletionService

Unchanged in shape; extended behavior only.

| Method | Behavior |
|--------|----------|
| `validate(command)` | Evaluates the rule catalog in documented order, returning `ValidatedChat` or the first `ValidationFailure` |
| `complete(validated)` | Consumes a `ValidatedChat`, returns the fixed `ChatCompletion`; ignores the controls because no model is connected |

`validate` is pure: it reads no configuration, opens no connection, and mutates
no shared state, so concurrent validation is isolated and repeatable by
construction rather than by convention.

## API Entities

### ChatCompletionRequestDto

| Attribute | Type | Required | Behavior |
|-----------|------|----------|----------|
| model | String | Yes | Must be non-empty; echoed exactly |
| messages | Vec\<MessageDto\> | Yes | At least one valid message |
| temperature | Option\<f64\> | No | Deserialized as a strict JSON number; `null` and every non-number type are refused |
| max_tokens | Option\<u32\> | No | Deserialized as a strict JSON integer; `null`, fractional, and every non-number type are refused |
| stream | bool | No; defaults false | `true` is refused last, after every other rule |

Unknown JSON fields are ignored. A repeated `temperature` or `max_tokens` is
refused rather than resolved.

This type replaces its derived deserializer with a manual one so that two rules
that a derived deserializer cannot express hold at the boundary: a supplied
`null` control is not an omitted control, and a repeated control is not a
last-value-wins override. The per-deserialization seen-key set is what makes the
duplicate rule safe under concurrency.

### MessageDto, ChatCompletionResponseDto, ChatCompletionChoiceDto, AssistantMessageDto

Unchanged from Phase 2. The success envelope gains no field, so a control is
validated and carried internally but never echoed.

### ApiErrorDto

Unchanged shape: exactly `code` and `message`, no `details` field, no wrapper, no
per-field list. One new row is added to the `ApiError` enum that renders through
it.

| Condition | Status | Code | Message |
|-----------|--------|------|---------|
| Body exceeds 1 MB | 413 | `payload_too_large` | `The request payload is too large.` |

The seven Phase 2 rows keep their status, code, and message unchanged, so the
contract stays stable for existing clients.

### Request body limit

| Constant | Value | Meaning |
|----------|-------|---------|
| `MAX_REQUEST_BODY_BYTES` | `1_048_576` | Maximum accepted whole request body; the boundary is inclusive and applies to the body, not to headers |

Declared as a fixed phase constant rather than configuration, because the
specification fixes the number and defers configurability to the security phase.
It lives in the API layer rather than beside the control ranges in the domain
because it is a property of the HTTP endpoint's transport, not an invariant of
the request model: a domain consumer can be handed a request built in-process,
where no body was ever received, and the control ranges still apply to it.

## Relationships

```text
ChatCompletionRequestDto ──maps──► CompleteChatCommand
CompleteChatCommand ──validated by──► MockChatCompletionService::validate
MockChatCompletionService::validate ──returns──► ValidatedChat ──wraps──► ChatRequest
ChatRequest ──carries──► temperature, max_tokens (each Option)

ChatCompletionRequestDto ──rejected by──► ApiError::PayloadTooLarge (body size stage)
ChatCompletionRequestDto ──rejected by──► ApiError::InvalidRequest (media type, syntax, type, duplicate)
CompleteChatCommand ──rejected by──► ValidationFailure (ranges, required fields)
CompleteChatCommand ──rejected by──► ApiError::UnsupportedFeature (stream)

ValidatedChat ──consumed by──► MockChatCompletionService::complete
MockChatCompletionService ──returns──► ChatCompletion
ChatCompletion ──maps to──► ChatCompletionResponseDto

ChatSizeLimitStage ──runs before──► JsonRejection mapping
ChatAdmissionMiddleware ──runs before──► ChatSizeLimitStage
```

## State Transitions

No lifecycle entity changes. The request state machine for the chat endpoint is:

```text
received
   |
   v
admitted?  ──no──► 503 not_ready
   |
   yes
   |
   v
body size within 1 MB?  ──no──► 413 payload_too_large
   |
   yes
   |
   v
media type is JSON?  ──no──► 415 unsupported_media_type
   |
   yes
   |
   v
body structurally readable, controls strictly typed and not repeated?  ──no──► 400 invalid_request
   |
   yes
   |
   v
model, messages, roles, content valid?  ──no──► 400 invalid_request
   |
   yes
   |
   v
temperature and max_tokens within range?  ──no──► 400 invalid_request
   |
   yes
   |
   v
stream not requested?  ──no──► 400 unsupported_feature
   |
   yes
   |
   v
validated ──► 200 mock completion
```

The first match wins; no request traverses two rejection paths, and repeating an
identical request traverses the same path. Every transition above is a pure
function of the request, which is what makes FR-015 and SC-006 achievable.

## Validation and Mapping Rules

1. The validated `ChatRequest` is the only request value application logic
   receives; it is produced only by `validate`.
2. An absent control stays absent; a present control is carried with the exact
   submitted value; no default is ever substituted.
3. `model` and each `content` must have non-zero length. Whitespace is
   preserved and is not rejected solely for being blank.
4. Message order is preserved exactly; no role-sequence, count, or per-message
   size rule applies in this phase.
5. `temperature` is present-and-in-range or absent. `max_tokens` is
   present-and-in-range or absent. There is no third state.
6. A control supplied as a non-number, including a numeric string, boolean,
   `null`, object, or array, is refused at the boundary and never becomes a
   server error.
7. A control supplied more than once is refused; no occurrence is chosen.
8. Unknown fields are ignored and never fail the request.
9. Request size is checked before media type and before any structural reading,
   and applies to the whole body.
10. Exactly one response is produced per request, chosen by the documented
    precedence order.
11. Failure responses contain exactly `code` and `message` and never contain the
    rejected value, prompt content, credentials, parser output, or internal
    diagnostics.
12. Validation performs no I/O, reads no credential, and contacts no provider,
    database, cache, or other external service.

## Traceability

- FR-001, FR-015, FR-016: the rule catalog, `ValidationFailure`, and the request
  state machine above.
- FR-002, FR-003: `ChatRequest` and `Message` field rules.
- FR-004, FR-005: `TEMPERATURE_MIN`/`MAX`, `MAX_TOKENS_MIN`/`MAX`, and the
  `ChatRequest` control invariants.
- FR-006, FR-007: the `Option` control representation and
  `ChatRequest::with_controls`.
- FR-008, FR-009: `ChatCompletionRequestDto` strict and duplicate-rejecting
  deserialization.
- FR-010, FR-011, FR-012: `MAX_REQUEST_BODY_BYTES`, the
  `payload_too_large` row, and the body-size stage ordering.
- FR-013: `StreamingRequested` evaluated last.
- FR-014: `ValidatedChat` opacity and by-value consumption in `complete`.
- FR-017, FR-018, FR-019: `ApiErrorDto` shape and the single new row.
- FR-020: ignored unknown fields in the manual deserializer.
- FR-021, FR-022: purity and isolation of `validate`.
- FR-023: the seven unchanged error rows.
- FR-024: unchanged lifecycle entities, verified by the existing suite.
- FR-025, FR-026, FR-027: the quality gates, the rule-by-rule checks, and the
  catalog plus contract documentation.
