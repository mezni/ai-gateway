# HTTP API Contract: HTTP Gateway Core

**Feature**: [spec.md](../spec.md)

**Date**: 2026-09-25

## 1. Scope and Conventions

- Base URL: `http://127.0.0.1:3000` by default.
- The address and port are runtime-configurable.
- Endpoints are unauthenticated in this local development phase.
- Request and response bodies use `application/json`.
- JSON object key order is not significant.
- Additional request fields are ignored until later specifications.
- Authentication, provider routing, streaming, usage reporting, request IDs,
  and dependency-aware readiness are not part of this contract.

## 2. Liveness

### Request

```http
GET /health
```

No body, credentials, or external dependency is required.

### Responses

Ready or shutting down:

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "status": "ok"
}
```

The endpoint reports only that the process can answer HTTP requests.

## 3. Readiness

### Request

```http
GET /ready
```

### Ready response

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "status": "ready"
}
```

### Not-ready response

```http
HTTP/1.1 503 Service Unavailable
Content-Type: application/json
```

```json
{
  "status": "not_ready"
}
```

Readiness is `ready` only after the listener is bound and startup is complete.
The production accept loop begins only after that transition; router-level tests
exercise the `Initializing` `ok`/`not_ready` pair. Readiness makes no claim about
providers, databases, caches, or other future dependencies.

## 4. Mock Chat Completion

### Request

```http
POST /v1/chat/completions
Content-Type: application/json
```

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

### Request fields

| Field | Type | Required | Rules |
|-------|------|----------|-------|
| model | string | Yes | Non-empty; echoed exactly; no route lookup |
| messages | array | Yes | At least one message |
| messages[].role | string | Yes | Exactly `system`, `user`, or `assistant` |
| messages[].content | string | Yes | Non-empty text |
| stream | boolean | No | Defaults to false; true is explicitly rejected |

`temperature`, `max_tokens`, and other future controls are not interpreted in
this phase. Unknown JSON fields are ignored. Non-empty strings are not trimmed,
so a whitespace-only value satisfies only the literal non-empty requirement.

### Successful response

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

The response is deterministic, contains exactly one non-streaming assistant
choice, requires no credentials, contacts no provider, and contains neither a
`usage` key nor a timestamp.

## 5. Validation Matrix

| Input condition | Result |
|-----------------|--------|
| Empty, malformed, or unreadable body | 400 `invalid_request` |
| Top-level value is not an object | 400 `invalid_request` |
| `model` missing, null, wrong type, or empty | 400 `invalid_request` |
| `messages` missing, null, wrong type, or empty | 400 `invalid_request` |
| Message is null or not an object | 400 `invalid_request` |
| Role missing, unknown, or wrong type | 400 `invalid_request` |
| Content missing, wrong type, or empty | 400 `invalid_request` |
| `stream` has a non-boolean value | 400 `invalid_request` |
| Structurally valid request with `stream: true` | 400 `unsupported_feature` |
| Missing or unsupported request media type | 415 `unsupported_media_type` |
| Unsupported method on a known path | 405 `method_not_allowed` |
| Unknown path | 404 `not_found` |
| New chat request while shutting down | 503 `not_ready` |
| Unexpected internal failure | 500 `internal_error` |

Validation precedence is routing/admission, media type, JSON structure, minimum
field validity, then streaming rejection. Thus a structurally invalid request
containing `stream: true` receives `invalid_request`.

## 6. Error Schema

Every request-processing error is a top-level object with exactly two fields:

```json
{
  "code": "machine-readable-code",
  "message": "Fixed human-readable message."
}
```

No `error` wrapper, `details`, request ID, source error, prompt, credential, or
implementation diagnostic is included. For a `HEAD` request, HTTP semantics
suppress the response body; the status and headers still identify the same
error condition, while body-returning methods carry the JSON object.

| Condition | Status | Code | Exact message |
|-----------|--------|------|---------------|
| Malformed request or missing minimum fields | 400 | `invalid_request` | `The chat request is invalid.` |
| Streaming requested | 400 | `unsupported_feature` | `Streaming is not supported.` |
| Unsupported request media type | 415 | `unsupported_media_type` | `The request media type is not supported.` |
| Unsupported method for a known path | 405 | `method_not_allowed` | `The request method is not allowed for this path.` |
| Unknown path | 404 | `not_found` | `The requested path was not found.` |
| New chat request after shutdown begins | 503 | `not_ready` | `The gateway is not accepting new chat requests.` |
| Unexpected internal failure | 500 | `internal_error` | `The gateway could not complete the request.` |

A 405 response also identifies allowed methods with an `Allow` header:
`GET` for `/health` and `/ready`, and `POST` for `/v1/chat/completions`.

## 7. Routing Contract

| Path | Allowed method |
|------|----------------|
| `/health` | `GET` |
| `/ready` | `GET` |
| `/v1/chat/completions` | `POST` |

Any unlisted path returns the 404 contract. Any other method on a listed path
returns the 405 contract. This includes `HEAD`: the Phase 2 contract exposes
only the methods in the table and does not rely on framework-generated aliases.
No automatic authentication, CORS, model-list, trailing-slash normalization,
or streaming route is added in this phase.

## 8. Shutdown Contract

When shutdown begins:

1. `/health` continues returning 200 `ok` while the listener remains available.
2. `/ready` returns 503 `not_ready`.
3. New chat requests return the 503 `not_ready` error contract.
4. Chat requests admitted before shutdown may finish.
5. The process closes the listener and exits no later than 10 seconds after the
   shutdown signal.

Readiness and error responses are operational/status contracts and are not
wrapped in the request-error shape.

## 9. Compatibility

This contract is the implemented Phase 2 subset of `docs/api.md`. Later phases
may add non-breaking fields or capabilities, but any breaking change requires a
new versioned contract and explicit migration documentation.
