# Contract: HTTP API — Provider Failure Extension

**Feature**: `specs/005-provider-abstraction`
**Date**: 2026-09-30
**Status**: Client-facing. This document **extends** an existing contract.
**Extends**: `specs/004-domain-validation/contracts/http-api.md` (superseded by this document)

---

## 1. Relationship to the existing contract

The client-facing contract established in Phase 3 is **frozen** and remains
authoritative for everything it already defines. This document adds exactly two
error rows and changes nothing else.

| Property | Phase 3 | Phase 4 |
|---|---|---|
| Distinct error codes | 8 | **10** |
| Error conditions tabulated | 15 | **17** |
| Conditions reachable by a client | 14 | **16** (`internal_error` is not client-triggerable) |
| Error body shape | `{"code","message"}`, exactly two keys | Unchanged |
| `details` field | Absent | Still absent |
| Wrapper object | Absent | Still absent |
| Routes | 3 | Still 3 |
| Success response keys | 4 (`id`, `object`, `model`, `choices`) | Unchanged |
| Request contract | Phase 3 validation | Unchanged |

"Codes" and "conditions" differ because one code covers several conditions:
`invalid_request` alone accounts for 8 of them. Both counts are stated
explicitly so the contract cannot be read as claiming ten *rows*.

**This is a deliberate, recorded expansion.** Phase 3 declared the eight-code
contract fixed, and FR-027 authorises widening it by exactly these two codes
because no existing code can represent a provider failure. Everything Phase 3
guarantees continues to hold, and every existing byte is unchanged.

---

## 2. New error rows

Two rows are added. They are the only change to the client-facing contract.

| Condition | Status | Code | Message |
|---|---|---|---|
| Provider cannot be reached, or declines the request | `502` | `provider_unavailable` | `The upstream provider is unavailable.` |
| Provider call exceeds the configured deadline | `504` | `provider_timeout` | `The upstream provider did not respond in time.` |

Both messages are **fixed strings**. Neither interpolates a provider name, a
provider status code, a provider message, a URL, a model name, or a credential.

### Existing rows, unchanged

| Condition | Status | Code | Message |
|---|---|---|---|
| Missing or invalid `model` | `400` | `invalid_request` | `The chat request is invalid.` |
| Empty, missing, or invalid `messages` | `400` | `invalid_request` | `The chat request is invalid.` |
| Unsupported message role | `400` | `invalid_request` | `The chat request is invalid.` |
| `temperature` or `max_tokens` is not a number | `400` | `invalid_request` | `The chat request is invalid.` |
| `temperature` outside 0.0-2.0 | `400` | `invalid_request` | `The chat request is invalid.` |
| `max_tokens` outside 1-4096 | `400` | `invalid_request` | `The chat request is invalid.` |
| Control supplied more than once | `400` | `invalid_request` | `The chat request is invalid.` |
| Body is not readable JSON | `400` | `invalid_request` | `The chat request is invalid.` |
| Streaming requested | `400` | `unsupported_feature` | `Streaming is not supported.` |
| Body larger than 1 MiB | `413` | `payload_too_large` | `The request payload is too large. |
| Unsupported request media type | `415` | `unsupported_media_type` | The request media type is not supported. |
| Unsupported method on a known path | `405` | `method_not_allowed` | The request method is not allowed for this path. |
| Unknown path | `404` | `not_found` | The requested path was not found. |
| New chat request after shutdown begins | `503` | `not_ready` | The gateway is not accepting new chat requests. |
| Provider response unusable or absent | `500` | `internal_error` | The gateway could not complete the request. |

---

## 3. Failure category to response mapping

| Provider failure category | Status | Code |
|---|---|---|
| `Unreachable` | `502` | `provider_unavailable` |
| `Refused` | `502` | `provider_unavailable` |
| `DeadlineExceeded` | `504` | `provider_timeout` |
| `UnusableResponse` | `500` | `internal_error` |
| `InvalidResponse` | `500` | `internal_error` |

### What a client may rely on

- `502` means the upstream provider was the problem.
- `504` means the upstream provider did not answer in time.
- `500` means the gateway could not make sense of what came back.

### What a client may NOT rely on

- The distinction between `Unreachable` and `Refused`. Both are `502`; the
  client cannot tell them apart, by design, because telling them apart would
  leak provider topology.
- The distinction between `UnusableResponse` and `InvalidResponse`. Both are
  `500`.
- Any provider-specific detail whatsoever. The contract guarantees none is
  present.

---

## 4. Examples

### Provider unreachable

```http
POST /v1/chat/completions
Content-Type: application/json

{"model":"general","messages":[{"role":"user","content":"Hello"}]}
```

```http
HTTP/1.1 502 Bad Gateway
Content-Type: application/json
```

```json
{
  "code": "provider_unavailable",
  "message": "The upstream provider is unavailable."
}
```

### Provider deadline exceeded

```http
HTTP/1.1 504 Gateway Timeout
Content-Type: application/json
```

```json
{
  "code": "provider_timeout",
  "message": "The upstream provider did not respond in time."
}
```

### Provider returned something unusable

```http
HTTP/1.1 500 Internal Server Error
Content-Type: application/json
```

```json
{
  "code": "internal_error",
  "message": "The gateway could not complete the request."
}
```

---

## 5. Invariants this extension must not break

1. **No provider detail leaks.** No provider status code, provider message,
   provider URL, provider model name, or credential appears in any response
   body. Verified by asserting the body equals the fixed string exactly.
2. **No validation code is reused for a provider fault.** A provider failure is
   never reported as `400 invalid_request`, which would blame the client.
3. **Validation still precedes any provider call.** An invalid request is
   refused before a provider is contacted, so a malformed request cannot cause
   a billable upstream call.
4. **The error body shape is unchanged.** Exactly two keys, no `details`, no
   wrapper, no per-field list.
5. **The success response is unchanged.** Exactly four top-level keys. No
   `usage` key is added; usage becomes client-visible in Phase 13.
6. **Provider selection is not client-controlled.** A client cannot choose a
   provider per request; selection is a startup configuration decision.
7. **A failure is never reported as success.** Partial or truncated provider
   output is never returned as a completion.

---

## 6. Precedence

Provider failure is the **last** stage. Everything the gateway can decide for
itself is decided first.

```text
1. route known?
2. method allowed?
3. gateway accepting new requests?
4. body within 1 MiB?
5. media type supported?
6. structure, types, duplicates, nulls valid?
7. required fields valid?
8. control ranges valid?
9. streaming not requested?
        |
        v
10. call the provider  <-- only now is a provider contacted
        |
        +-- success        -> 200 with the four-key envelope
        +-- unreachable    -> 502 provider_unavailable
        +-- refused        -> 502 provider_unavailable
        +-- deadline       -> 504 provider_timeout
        +-- unusable body  -> 500 internal_error
        +-- no completion  -> 500 internal_error
```

A request that fails any of stages 1-9 never reaches stage 10.
