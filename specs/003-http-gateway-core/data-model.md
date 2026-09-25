# Data Model: HTTP Gateway Core

**Feature**: [spec.md](spec.md)

**Date**: 2026-09-25

This phase adds process-local lifecycle, configuration, transport DTOs, and one
mock completion. No durable storage or provider entity is created.

## Domain Entities

### ChatRequest

Existing provider-independent request mapped from the API DTO.

| Attribute | Type | Rules |
|-----------|------|-------|
| model | String | Non-empty gateway model identifier; no registry lookup in this phase |
| messages | Vec<Message> | At least one message, preserved in order |

### Message

| Attribute | Type | Rules |
|-----------|------|-------|
| role | MessageRole | Exactly `system`, `user`, or `assistant` |
| content | String | Non-empty text; no trimming or size policy in this phase |

### MessageRole

| Variant | Wire value |
|---------|------------|
| System | `system` |
| User | `user` |
| Assistant | `assistant` |

### ChatResponse

Normalized application/domain completion result.

| Attribute | Type | Rules |
|-----------|------|-------|
| content | String | Mock value is exactly `This is a mocked chat completion.` |
| usage | Option<Usage> | `None` means usage was not measured; the mock always returns `None` |

### Usage

| Attribute | Type | Rules |
|-----------|------|-------|
| prompt_tokens | u64 | Input token count |
| completion_tokens | u64 | Output token count |
| total_tokens | u64 | Maintained as prompt + completion when present |

The existing constructor remains compatible by accepting `Usage` and storing
`Some(usage)`. A separate no-usage constructor is used by the mock.

## Application Entities

### CompleteChatCommand

The inward-facing input created by the API after structural JSON parsing.

| Attribute | Type | Rules |
|-----------|------|-------|
| model | String | Validated as non-empty by the application |
| messages | Vec<IncomingMessage> | Validated as non-empty by the application |

### IncomingMessage

| Attribute | Type | Rules |
|-----------|------|-------|
| role | String | Converted by the application to `system`, `user`, or `assistant` |
| content | String | Validated as non-empty by the application |

### ValidatedChat

An opaque application value produced after minimum validation. It wraps the
domain `ChatRequest` internally so the API can carry the validated command
without importing or inspecting the domain type.

### ChatCompletion

The transport-neutral application result consumed by the API.

| Attribute | Type | Rules |
|-----------|------|-------|
| content | String | Fixed mock assistant text |

Usage is not exposed in the Phase 2 application projection. The application
uses the domain `ChatResponse` internally with `usage: None` rather than inventing
measurements.

### MockChatCompletionService

A provider-free deterministic use case.

| Attribute | Type | Rules |
|-----------|------|-------|
| state | None | Stateless and safe to share across requests |

`validate` converts a `CompleteChatCommand` into `ValidatedChat` and returns the
fixed public invalid-request error when minimum rules fail. `complete` consumes
`ValidatedChat`, constructs the domain request/response internally, and returns
a `ChatCompletion`. It performs no I/O and requires no credentials.

### GatewayPhase

| Value | Observable behavior |
|-------|---------------------|
| Initializing | Router tests observe `/health` as `ok` and `/ready` as `not_ready`; chat is not admitted; the production listener is bound but Axum is not accepting/serving before Ready |
| Ready | `/health` is `ok`; `/ready` is `ready`; chat is admitted |
| ShuttingDown | `/health` is `ok`; `/ready` is `not_ready`; new chat is rejected |
| Stopped | Listener is closed; no endpoint is externally reachable |

### GatewayLifecycle

Owns the current phase and admission accounting.

| State | Type | Rules |
|-------|------|-------|
| phase | Atomic lifecycle value | Forward-only transitions; shutdown is idempotent |
| in_flight | Atomic count | Increments only when a request is admitted while Ready |
| in_flight_updates | Tokio watch sender/receiver | Lets shutdown await zero without polling or blocking runtime workers |

Admission is rechecked around the count increment so a request is either admitted
before the shutdown transition or receives the 503 `not_ready` contract.

### ChatAdmissionGuard

An RAII guard created by chat-route middleware before JSON extraction for an
admitted request. Decrementing and notifying occur in `Drop`, covering successful
responses, application errors, incomplete bodies, and client cancellation.

### AppState

| Attribute | Type | Purpose |
|-----------|------|---------|
| version | &'static str | Existing package-version seam |
| lifecycle | Arc<GatewayLifecycle> | Shared readiness and admission state |
| chat_service | MockChatCompletionService | Deterministic application use case |

The existing `AppState::new(version)` seam remains available and starts in
`Initializing`; explicit lifecycle methods drive readiness and shutdown.

## Configuration Entity

### ServerConfig

| Attribute | Type | Default | Rules |
|-----------|------|---------|-------|
| host | IpAddr | `127.0.0.1` | IPv4 or IPv6 literal from `AI_GATEWAY_HOST` |
| port | u16 | `3000` | Non-zero value from `AI_GATEWAY_PORT` |

The listener address is `host:port`. The shutdown allowance is a fixed Phase 2
constant of 10 seconds and is not externally configurable.

## API DTOs

### ChatCompletionRequestDto

| Attribute | Type | Required | Behavior |
|-----------|------|----------|----------|
| model | String | Yes | Must be non-empty and is echoed |
| messages | Vec<MessageDto> | Yes | Must contain at least one valid message |
| stream | bool | No; defaults false | `true` returns `unsupported_feature` |

Other JSON fields are ignored until their later specifications. Missing,
malformed, null, and wrong-type required fields fail deserialization or mapping.

### MessageDto

| Attribute | Type | Required | Behavior |
|-----------|------|----------|----------|
| role | String | Yes | Maps to application `IncomingMessage`; application converts supported values to domain `MessageRole` |
| content | String | Yes | Must be non-empty |

### ChatCompletionResponseDto

| Attribute | Type | Value |
|-----------|------|-------|
| id | String | `chat_mock` |
| object | String | `chat.completion` |
| model | String | Exact validated request model |
| choices | Vec<ChatCompletionChoiceDto> | Exactly one choice |

### ChatCompletionChoiceDto

| Attribute | Type | Value |
|-----------|------|-------|
| index | u32 | `0` |
| message | AssistantMessageDto | Fixed assistant response |
| finish_reason | String | `stop` |

### AssistantMessageDto

| Attribute | Type | Value |
|-----------|------|-------|
| role | String | `assistant` |
| content | String | `This is a mocked chat completion.` |

No `usage` or `created` field exists in the success DTO.

### HealthResponseDto / ReadinessResponseDto

| Entity | Field | Values |
|--------|-------|--------|
| HealthResponseDto | status | `ok` |
| ReadinessResponseDto | status | `ready` or `not_ready` |

### ApiErrorDto

| Attribute | Type | Rule |
|-----------|------|------|
| code | String | Fixed machine-readable code |
| message | String | Fixed safe human-readable message |

The object contains exactly these two fields.

## Relationships

```text
ServerConfig ──configures──► TCP listener

AppState ──owns──► GatewayLifecycle
AppState ──owns──► MockChatCompletionService

ChatCompletionRequestDto ──maps──► CompleteChatCommand
CompleteChatCommand ──validated by──► ValidatedChat ──wraps──► ChatRequest
ChatRequest ──processed by──► MockChatCompletionService
MockChatCompletionService ──returns──► ChatCompletion
ChatCompletion ──maps to──► ChatCompletionResponseDto ──contains──► one choice

ChatAdmissionMiddleware ──uses──► GatewayLifecycle
GatewayLifecycle ──reports──► ReadinessResponseDto
```

## State Transitions

```text
Initializing ──listener bound, router built, publish barrier passed──► Ready
Initializing ──shutdown signal──► ShuttingDown
Ready ──shutdown signal──► ShuttingDown
ShuttingDown ──drained or deadline reached──► Stopped
```

- Readiness is true only in `Ready`.
- The production accept loop is not driven while `Initializing`; router tests
  exercise its `ok`/`not_ready` behavior.
- `/health` remains `ok` in every externally reachable phase.
- New chat admission is allowed only in `Ready`.
- A repeated shutdown signal does not move the process backward.
- No transition returns to an earlier phase.

## Validation and Mapping Rules

1. JSON structure is validated before application execution.
2. The API maps structural DTO values to `CompleteChatCommand` without importing
   domain types.
3. The application validates model/message rules and maps the command to
   `ChatRequest`; model, content, role, and message order are preserved.
4. Model and each content value must have non-zero string length; whitespace is
   preserved and is not rejected solely for being blank.
5. `stream` is checked only after the application command validates.
6. No API-only field enters the domain; `stream` is consumed by the API.
7. The mock ignores message content for response generation but does not mix it
   across requests.
8. The domain result has no usage and the success DTO has no usage field.
9. Request-processing errors never serialize source errors, prompts, stack data,
   credentials, or diagnostic details.

## Traceability

- FR-001..FR-007: routes, DTOs, mapping, and configuration entities.
- FR-008..FR-010: mock service and success DTO.
- FR-011..FR-014: validation precedence and `ApiErrorDto`.
- FR-015: stateless service and request-owned DTO/domain values.
- FR-016: lifecycle, admission guard, and shutdown transitions.
- FR-017: `ServerConfig`.
- FR-018..FR-020: automated coverage and quickstart/documentation deliverables.
