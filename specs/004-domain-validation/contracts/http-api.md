# HTTP API Contract: Domain Model Validation

**Feature**: [spec.md](../spec.md) | **Rules**: [validation-rules.md](validation-rules.md) | **Model**: [data-model.md](../data-model.md)

**Date**: 2026-09-30

This contract states the `POST /v1/chat/completions` wire behavior implemented in
this phase. It supersedes the Phase 2 chat-endpoint sections of
`specs/003-http-gateway-core/contracts/http-api.md`; every part of that contract
this phase does not change — liveness, readiness, routing, method handling,
shutdown behavior, and the seven unchanged error rows — continues to apply
unchanged.

## 1. Scope and Conventions

- Base URL: `http://127.0.0.1:3000` by default; the address and port remain
  runtime-configurable.
- The endpoint is unauthenticated and local in this phase.
- Request and response bodies use `application/json`.
- JSON object key order is not significant.
- Additional request fields are ignored.
- Nothing in this contract introduces an endpoint, a version, or a new
  client-visible success field.

## 2. Request

```http
POST /v1/chat/completions
Content-Type: application/json
```

### 2.1 Canonical request

```json
{
  "model": "mock-model",
  "messages": [
    {
      "role": "user",
      "content": "Hello"
    }
  ]
}
```

### 2.2 Request with generation controls

```json
{
  "model": "mock-model",
  "messages": [
    {
      "role": "user",
      "content": "Hello"
    }
  ],
  "temperature": 0.7,
  "max_tokens": 500
}
```

### 2.3 Request fields

| Field | Type | Required | Rules |
|-------|------|----------|-------|
| `model` | string | Yes | Non-empty; echoed exactly; no route lookup |
| `messages` | array | Yes | At least one message |
| `messages[].role` | string | Yes | Exactly `system`, `user`, or `assistant` |
| `messages[].content` | string | Yes | Non-empty text |
| `temperature` | number | No | `0.0`–`2.0` inclusive; a JSON number only |
| `max_tokens` | integer | No | `1`–`4096` inclusive; a JSON integer only |
| `stream` | boolean | No | Defaults to false; `true` is explicitly refused |

The complete rule set, boundary values, and precedence are specified in
[validation-rules.md](validation-rules.md). In summary:

- `temperature` and `max_tokens` are optional. Omitting them is valid and never
  an error.
- A control must be supplied as a JSON number, or a JSON integer for
  `max_tokens`. A numeric string, boolean, `null`, object, or array is refused
  with `invalid_request`, never with a server error. In particular `null` is a
  refusal, not an omission.
- A `max_tokens` value must be an integer. Fractional and exponent forms are
  refused.
- A control supplied more than once is refused. No occurrence is silently chosen.
- Unknown extra fields are ignored and never fail the request.
- Non-empty strings are not trimmed, so a whitespace-only value satisfies the
  literal non-empty requirement.
- No per-field content limit, message count limit, or per-message size limit
  applies. The whole-body limit below is the only size rule.

### 2.4 Request size

The whole request body is limited to **1 048 576 bytes (1 MB)**.

- A body of exactly 1 048 576 bytes is accepted; the boundary is inclusive.
- A body of 1 048 577 bytes is refused with 413 `payload_too_large` and no
  completion.
- The limit applies to the body, not to headers.
- A request with a declared body length above the limit is refused without the
  body being read. A request that does not declare a length is bounded while it
  is read, so a body that only reveals its size as it arrives is still refused.
- A refused or abandoned oversized body does not affect the gateway: it stays
  healthy and continues serving later requests.
- The limit is fixed in this phase and is not configurable.

## 3. Successful Response

Unchanged from Phase 2:

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "id": "chat_mock",
  "object": "chat.completion",
  "model": "mock-model",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "This is a mocked chat completion."
      },
      "finish_reason": "stop"
    }
  ]
}
```

The response is deterministic and contains exactly one non-streaming assistant
choice. It carries neither a `usage` key nor a timestamp, and this phase adds no
field that echoes the generation controls. Because no model is connected, the
response is identical with and without `temperature` and `max_tokens`; it never
implies that a control influenced the result.

## 4. Validation Precedence

Evaluated in this order; the first failure determines the response and no later
rule is evaluated.

| # | Stage | Response on failure |
|---|-------|---------------------|
| 1 | Route | 404 `not_found` |
| 2 | Method | 405 `method_not_allowed` |
| 3 | Admission (lifecycle) | 503 `not_ready` |
| 4 | Request size | 413 `payload_too_large` |
| 5 | Media type | 415 `unsupported_media_type` |
| 6 | Structural readability, control typing, repeated controls | 400 `invalid_request` |
| 7 | Required-field validity | 400 `invalid_request` |
| 8 | Control-range validity | 400 `invalid_request` |
| 9 | Streaming | 400 `unsupported_feature` |

Note the two rows that differ from a naive reading: size precedes media type,
because an oversized body cannot be validated without reading it; and streaming
is last, so a request that is invalid on its own merits reports
`invalid_request` even when it also asks for a stream.

## 5. Validation Matrix

| Input condition | Result |
|-----------------|--------|
| Body larger than 1 MB | 413 `payload_too_large` |
| Missing or unsupported request media type | 415 `unsupported_media_type` |
| Malformed, empty, or unreadable body | 400 `invalid_request` |
| Top-level value is not an object | 400 `invalid_request` |
| `model` missing, null, wrong type, or empty | 400 `invalid_request` |
| `messages` missing, null, wrong type, or empty | 400 `invalid_request` |
| Message is null or not an object | 400 `invalid_request` |
| Role missing, unknown, wrong type, or differently cased | 400 `invalid_request` |
| Content missing, wrong type, or empty | 400 `invalid_request` |
| `temperature` is a string, boolean, `null`, object, or array | 400 `invalid_request` |
| `temperature` below `0.0` or above `2.0` | 400 `invalid_request` |
| `max_tokens` is a string, boolean, `null`, object, array, or non-integer number | 400 `invalid_request` |
| `max_tokens` is `0`, negative, or above `4096` | 400 `invalid_request` |
| `temperature` or `max_tokens` supplied more than once | 400 `invalid_request` |
| `stream` has a non-boolean value | 400 `invalid_request` |
| Structurally valid request with `stream: true` | 400 `unsupported_feature` |
| Valid request with in-range or absent controls | 200 with the mock completion |
| Unknown extra fields present | 200; the fields are ignored |

## 6. Error Schema

Unchanged from Phase 2 and extended by one row. Every request-processing error is
a top-level object with exactly two fields:

```json
{
  "code": "machine-readable-code",
  "message": "Fixed human-readable message."
}
```

| Condition | Status | Code | Exact message |
|-----------|--------|------|---------------|
| Body larger than 1 MB | 413 | `payload_too_large` | `The request payload is too large.` |
| Malformed request, invalid control, or missing/invalid minimum fields | 400 | `invalid_request` | `The chat request is invalid.` |
| Streaming requested | 400 | `unsupported_feature` | `Streaming is not supported.` |
| Unsupported request media type | 415 | `unsupported_media_type` | `The request media type is not supported.` |
| Unsupported method for a known path | 405 | `method_not_allowed` | `The request method is not allowed for this path.` |
| Unknown path | 404 | `not_found` | `The requested path was not found.` |
| New chat request after shutdown begins | 503 | `not_ready` | `The gateway is not accepting new chat requests.` |
| Unexpected internal failure | 500 | `internal_error` | `The gateway could not complete the request.` |

A failure response never contains the rejected value, prompt content, a
credential, parser output, or any internal diagnostic. There is no `details`
field and no per-field error list. A 405 response continues to carry the
`Allow` header, and a `HEAD` request continues to receive the status and headers
without a body.

The eight `invalid_request` conditions in §5 deliberately share one status, code,
and message: identifying the offending field is not part of this phase, which
keeps the contract stable for existing clients.

## 7. Routing Contract

Unchanged from Phase 2.

| Path | Allowed method |
|------|----------------|
| `/health` | `GET` |
| `/ready` | `GET` |
| `/v1/chat/completions` | `POST` |

`HEAD` is not an alias on any path. No authentication, CORS, model-list,
trailing-slash normalization, streaming route, or additional endpoint is added.

## 8. Shutdown Contract

Unchanged from Phase 2: `/health` stays `ok` while the listener is available,
`/ready` becomes `not_ready`, new chat requests receive 503 `not_ready`, admitted
chat requests may finish, and the process closes the listener no later than 10
seconds after the shutdown signal. A request admitted before shutdown and later
refused by validation still releases its admission slot.

## 9. Compatibility

- **Breaking for clients:** none. Every Phase 2 request that was accepted is
  still accepted, except a request carrying `temperature` or `max_tokens` with a
  value Phase 2 ignored — those values are now validated, which is the intent of
  this phase.
- **Breaking for clients:** a body larger than 1 MB is now refused. Phase 2
  inherited the framework's implicit transport default for this endpoint; this
  phase replaces it with the documented 1 MB bound.
- **Additive:** one new error row, `payload_too_large`, distinguishable from
  every other client error by its status and code.
- **Unchanged:** the success envelope, the routing table, the seven existing
  error rows, liveness, readiness, and graceful shutdown.

Any future breaking change requires a new versioned contract and explicit
migration documentation.
