# Validation Rule Catalog: Domain Model Validation

**Feature**: [spec.md](../spec.md) | **Wire contract**: [HTTP API](http-api.md) | **Model**: [data-model.md](../data-model.md)

**Date**: 2026-09-30

This catalog is the authoritative statement of what the gateway validates, in
what order, and what each failure returns. It is the user-facing source of truth
required by FR-027: a client developer can predict any response from this
document without reading the gateway's source code.

Scope: `POST /v1/chat/completions`. Liveness, readiness, routing, media type,
method, and lifecycle rules are unchanged from Phase 2 and are restated only
where they participate in the precedence order.

## 1. Supported Values

| Field | Required | Accepted values | Notes |
|-------|----------|-----------------|-------|
| `model` | Yes | Any non-empty string | Preserved exactly, including surrounding whitespace. Not required to be registered or routable. |
| `messages` | Yes | Array with at least one entry | Submitted order preserved. |
| `messages[].role` | Yes | `system`, `user`, `assistant` | Exact match. No other value, casing, or padding is accepted. |
| `messages[].content` | Yes | Any non-empty string | Preserved exactly. A whitespace-only value satisfies the non-empty requirement. |
| `temperature` | No | JSON number in `0.0`–`2.0` inclusive | A JSON number only. |
| `max_tokens` | No | JSON integer in `1`–`4096` inclusive | A JSON integer only. |
| `stream` | No | `false`, or absent | `true` is refused; see rule `STREAM-001`. |
| request body | Yes | At most 1 048 576 bytes (1 MB) | Whole body, inclusive boundary. Applies to the body, not to headers. |

Anything else in the body is an unknown field and is ignored.

### 1.1 Boundary Values

| Value | Outcome |
|-------|---------|
| `temperature` exactly `0.0` | Accepted |
| `temperature` exactly `2.0` | Accepted |
| `temperature` `2.0001` (or any value above `2.0`) | Refused — `CTRL-002` |
| `temperature` below `0.0` | Refused — `CTRL-002` |
| `max_tokens` exactly `1` | Accepted |
| `max_tokens` exactly `4096` | Accepted |
| `max_tokens` `4097` (or any value above `4096`) | Refused — `CTRL-003` |
| `max_tokens` `0` or negative | Refused — `CTRL-003` |
| body of exactly 1 048 576 bytes | Accepted — `SIZE-001` does not fire |
| body of 1 048 577 bytes | Refused — `SIZE-001` |

## 2. Rule Identifiers

| ID | Rule | Failure response |
|----|------|------------------|
| `ROUTE-001` | Path is `/v1/chat/completions` | 404 `not_found` |
| `METHOD-001` | Method is `POST` | 405 `method_not_allowed` |
| `ADMIT-001` | The gateway is `Ready` | 503 `not_ready` |
| `SIZE-001` | Whole request body is at most 1 048 576 bytes | 413 `payload_too_large` |
| `TYPE-001` | Request media type is JSON | 415 `unsupported_media_type` |
| `PARSE-001` | The body is a readable JSON object | 400 `invalid_request` |
| `TYPE-002` | `temperature`, when present, is a JSON number | 400 `invalid_request` |
| `TYPE-003` | `max_tokens`, when present, is a JSON integer | 400 `invalid_request` |
| `TYPE-004` | `stream`, when present, is a JSON boolean | 400 `invalid_request` |
| `UNIQ-001` | `temperature` appears at most once | 400 `invalid_request` |
| `UNIQ-002` | `max_tokens` appears at most once | 400 `invalid_request` |
| `FIELD-001` | `model` is a non-empty string | 400 `invalid_request` |
| `FIELD-002` | `messages` is present and contains at least one message | 400 `invalid_request` |
| `FIELD-003` | Every message is an object with `role` and `content` | 400 `invalid_request` |
| `FIELD-004` | Every `role` is `system`, `user`, or `assistant` | 400 `invalid_request` |
| `FIELD-005` | Every `content` is a non-empty string | 400 `invalid_request` |
| `CTRL-001` | `temperature`, when present, is within `0.0`–`2.0` | 400 `invalid_request` |
| `CTRL-002` | `temperature`, when present, is a JSON number | 400 `invalid_request` |
| `CTRL-003` | `max_tokens`, when present, is within `1`–`4096` | 400 `invalid_request` |
| `STREAM-001` | `stream` is absent or `false` | 400 `unsupported_feature` |

`CTRL-002` duplicates `TYPE-002` in observable behavior by design: the catalog
records the rule at both the boundary stage that enforces it and the ordered
rule set that documents its position. Both produce the same response, so the
duplication is documentation, not a second decision.

## 3. Precedence Order

Rules are evaluated in this fixed order. The **first** rule that fails determines
the response; no later rule is evaluated, and no request produces two responses.

| # | Stage | Rules | Response on failure |
|---|-------|-------|---------------------|
| 1 | Route | `ROUTE-001` | 404 `not_found` |
| 2 | Method | `METHOD-001` | 405 `method_not_allowed` |
| 3 | Admission | `ADMIT-001` | 503 `not_ready` |
| 4 | Request size | `SIZE-001` | 413 `payload_too_large` |
| 5 | Media type | `TYPE-001` | 415 `unsupported_media_type` |
| 6 | Structural readability | `PARSE-001`, `TYPE-002`, `TYPE-003`, `TYPE-004`, `UNIQ-001`, `UNIQ-002` | 400 `invalid_request` |
| 7 | Required-field validity | `FIELD-001`..`FIELD-005` | 400 `invalid_request` |
| 8 | Control-range validity | `CTRL-001`, `CTRL-002`, `CTRL-003` | 400 `invalid_request` |
| 9 | Streaming | `STREAM-001` | 400 `unsupported_feature` |

Consequences a client can rely on:

- An oversized body reports `payload_too_large` even when its media type is also
  wrong, because the body cannot be validated without reading it.
- A request that violates a required field *and* asks for streaming reports
  `invalid_request`, not `unsupported_feature`.
- A request that supplies a mistyped control *and* an out-of-range other
  control reports the single `invalid_request` row; the two are
  indistinguishable to the client, by design.

## 4. Response Contract

Every failure below is a top-level object with **exactly** two fields:

```json
{
  "code": "machine-readable-code",
  "message": "Fixed human-readable message."
}
```

There is no `details` field, no error wrapper, no per-field list, and no
request identifier. The messages are fixed: a client may match on them.

| Condition | Status | Code | Exact message |
|-----------|--------|------|---------------|
| Missing or invalid `model` | 400 | `invalid_request` | `The chat request is invalid.` |
| Empty, missing, or invalid `messages` | 400 | `invalid_request` | `The chat request is invalid.` |
| Unsupported message role | 400 | `invalid_request` | `The chat request is invalid.` |
| Empty message content | 400 | `invalid_request` | `The chat request is invalid.` |
| `temperature` or `max_tokens` is not a number | 400 | `invalid_request` | `The chat request is invalid.` |
| `temperature` outside `0.0`–`2.0` | 400 | `invalid_request` | `The chat request is invalid.` |
| `max_tokens` outside `1`–`4096` | 400 | `invalid_request` | `The chat request is invalid.` |
| Control supplied more than once | 400 | `invalid_request` | `The chat request is invalid.` |
| Body is not readable JSON | 400 | `invalid_request` | `The chat request is invalid.` |
| Streaming requested | 400 | `unsupported_feature` | `Streaming is not supported.` |
| Request body larger than 1 MB | 413 | `payload_too_large` | `The request payload is too large.` |
| Unsupported request media type | 415 | `unsupported_media_type` | `The request media type is not supported.` |
| Unsupported method on a known path | 405 | `method_not_allowed` | `The request method is not allowed for this path.` |
| Unknown path | 404 | `not_found` | `The requested path was not found.` |
| New chat request after shutdown begins | 503 | `not_ready` | `The gateway is not accepting new chat requests.` |
| Unexpected internal failure | 500 | `internal_error` | `The gateway could not complete the request.` |

## 5. What a Failure Response Never Contains

A failure response never contains, in any form:

- the submitted value that caused the failure, including the model, a
  generation control, or a role;
- any message or prompt content;
- any credential;
- parser output, deserializer text, field names, or type-mismatch descriptions;
- stack traces, source locations, or internal identifiers;
- a count, length, or offset describing the request.

## 6. Accepted Requests

A request that satisfies every rule is accepted and produces the deterministic
mock completion. Acceptance is independent of the generation controls: a request
with in-range controls, a request without them, and a request that repeats the
same content return byte-identical responses.

The controls are validated and carried into the internal request. They are not
echoed in the response and do not change the mock output, because no model is
connected. A response never implies that a control influenced it.

## 7. Out of Scope

This catalog does not cover, and this phase does not implement:

- per-field content limits, a maximum message count, or per-message size limits
  (deferred to the domain-validation follow-up);
- a configurable body size limit or hardening of the size limit (deferred to the
  security phase);
- per-model, per-tenant, or policy-based control ranges (deferred to the routing
  and policy phases);
- model registration or routability;
- authentication, authorization, rate limiting, persistence, or caching;
- streaming responses, which remain unimplemented and explicitly refused.
