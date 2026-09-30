# Data Model: Provider Abstraction

**Feature**: `specs/005-provider-abstraction`
**Date**: 2026-09-30
**Source**: [spec.md](spec.md), [research.md](research.md)

This feature adds three new concepts and changes one existing one. Every
client-facing type is unchanged.

---

## 1. Provider

A source of chat completions the gateway can be pointed at.

Represents the domain-level `Provider` that already exists in
`src/domain/catalog.rs` as a vendor-neutral concept. This feature gives it the
operational meaning it lacked: something the gateway can actually select and
call.

### Fields

| Field | Type | Required | Rules |
|---|---|---|---|
| `id` | `String` | yes | Stable identifier, non-empty. Must be lowercase ASCII with no whitespace so it is safe as an environment value and as a log field. This is the value the operator selects by. |
| `name` | `String` | yes | Human-readable label. Free text, never used for selection. |
| `enabled` | `bool` | yes | Whether the provider may be selected. A disabled provider exists but cannot be chosen. |
| `requires_credentials` | `bool` | yes | Whether the provider needs a credential from the environment before it can be used. Drives startup validation. |

### Rules

- **FR-006**: exactly one provider ships enabled by default — the
  deterministic provider. It has `enabled = true` and
  `requires_credentials = false`.
- **FR-006a**: any additional provider is a test double. It is registered in
  test builds only and MUST NOT be reachable from a shipped configuration.
- **FR-005**: selection is by `id`, resolved once at startup.
- **FR-007**: a selection naming an unknown `id`, or a known but `enabled =
  false` `id`, fails startup with a message containing the requested value.
- **FR-008**: the resolved `id` is reported through the existing readiness
  surface and in the startup-refusal message.

### Relationships

- One `Provider` is selected per process; the selection holds an `Arc` to that
  provider's implementation.
- A `Provider` implementation consumes a `ProviderCompletionRequest` and
  produces either a `ChatResponse` or a `ProviderFailure`.

---

## 2. ProviderCompletionRequest

The provider-agnostic description of what to produce.

This is **not** a new type. It is the existing `ChatRequest` from
`src/domain/chat.rs`, reached by the same validated path built in Phase 3. The
provider contract accepts it directly.

### Fields (existing, unchanged)

| Field | Type | Rules |
|---|---|---|
| `model` | `String` | Validated non-empty by Phase 3. Provider-agnostic. |
| `messages` | `Vec<Message>` | Non-empty, each with a valid role and non-empty content. |
| `temperature` | `Option<f64>` | Inclusive `0.0`–`2.0`, or unspecified. |
| `max_tokens` | `Option<u32>` | Inclusive `1`–`4096`, or unspecified. |

### Rules

- **FR-003**: no provider-native type appears in this request. An adapter that
  needs a provider-specific field must derive it locally and MUST NOT add it
  here.
- **FR-004**: this type MUST NOT gain provider-specific fields to accommodate
  one provider.
- **FR-022**: provider-native request structures stay inside the adapter.
- Unspecified controls stay unspecified. An adapter MUST NOT substitute a
  provider default silently in a way the gateway then reports as if the client
  had asked for it.

---

## 3. ProviderFailure

A categorized outcome that is not a completion. New type.

### Variants

| Variant | Meaning | Trigger | Client-facing code |
|---|---|---|---|
| `Unreachable` | The provider could not be contacted | Connection refused, DNS failure, TLS failure, no route | `502 provider_unavailable` |
| `Refused` | The provider was reached and declined | Provider returned a rejection for the request | `502 provider_unavailable` |
| `DeadlineExceeded` | The call exceeded its deadline | The bounded wait elapsed first | `504 provider_timeout` |
| `UnusableResponse` | Success, but the body could not be interpreted | Malformed payload, unexpected structure, wrong content encoding | `500 internal_error` |
| `InvalidResponse` | Interpretable, but no completion present | Required completion field absent or empty | `500 internal_error` |

### Rules

- **FR-010**: the five categories are the branching surface. Callers MUST NOT
  branch on message text.
- **FR-012**: this type MUST NOT carry provider status codes, provider message
  text, or credentials. It carries the category and a fixed gateway-authored
  message, so provider detail cannot reach a client by construction rather
  than by filtering.
- **FR-013**: a failure is never reported as a successful completion.
- **FR-018**: constructing a failure has no side effect on the gateway.
- No variant maps to a validation code. A provider fault MUST NOT be reported
  as `invalid_request`, which would blame the client.

### Fixed client-facing messages

| Code | Message |
|---|---|
| `502 provider_unavailable` | `The upstream provider is unavailable.` |
| `504 provider_timeout` | `The upstream provider did not respond in time.` |

Both are fixed strings with no interpolation (FR-027). The existing
`500 internal_error` keeps its current message, `The gateway could not complete
the request.`

---

## 4. ChatResponse (existing, unchanged)

Reused as-is. Its `usage` field is already `Option<Usage>`.

### Rules

- **FR-014**: the gateway MUST NOT invent token counts. It passes through what
  the provider reported. Absence is already representable and is what the
  current mock uses.
- **FR-016**: the client-facing response is built from this and is unchanged by
  this feature. It carries exactly four top-level keys — `id`, `object`,
  `model`, `choices` — and no `usage` key, which an existing test asserts.
  Usage becomes client-visible in Phase 13.
- **FR-004**: this type MUST NOT gain provider-specific fields.

---

## 5. ProviderSelection (resolved at startup)

The outcome of configuration resolution. Not a runtime type the request path
passes around; a startup artifact.

### Fields

| Field | Type | Rules |
|---|---|---|
| `provider_id` | `String` | The `id` that was selected. Reported at readiness. |
| `deadline_ms` | `u64` | The bounded provider call deadline, validated at startup. |

### Rules

- **FR-005**: resolved once at startup from the environment, not per request.
- **FR-011**: `deadline_ms` is enforced by the application layer wrapping the
  provider call, so an adapter cannot omit it.
- **FR-028**: sourced from process environment variables only. No configuration
  file.
- An unparsable or non-positive `deadline_ms` fails startup.
- Per-request selection is out of scope until Phase 6.

---

## State transitions

Provider selection is resolved once and does not change while the process runs.

```text
                 startup
                    |
                    v
        [environment read]
           |          |
     invalid value   valid value
           |          |
           v          v
      FAIL FAST    [resolve by id]
      (name the       |         |
       provider)  unknown id   disabled
                       |         |
                       v         v
                   FAIL FAST  FAIL FAST
                                 |
                                 v
                          [READY to serve]
                                 |
                    request arrives
                                 |
                                 v
                      [bounded call]
                       |     |      |
                    success timeout  failure
                       |     |      |
                       v     v      v
                  [response] [504] [502 or 500]
```

No transition occurs per request, because selection is fixed at startup. A
request never moves the provider between states.

---

## Validation rules summary

| Rule | Where enforced | Failure mode |
|---|---|---|
| Provider `id` non-empty, lowercase, no whitespace | startup, when reading environment | fail fast, name the value |
| Selection names a known provider | startup | fail fast, name the value |
| Selection names an enabled provider | startup | fail fast, name the value |
| Deadline is a positive integer | startup | fail fast, name the variable |
| `ProviderFailure` carries no provider detail | construction, by type shape | impossible to express |
| Request stays within Phase 3 bounds | Phase 3 pipeline, before any provider call | existing `400 invalid_request`, provider never called |
| Request body within 1 MiB | Phase 3 middleware, before any provider call | existing `413 payload_too_large`, provider never called |

The last two rows matter for cost as well as correctness: an invalid request
MUST be refused before a provider call is attempted, so a malformed client
request can never produce a billable upstream call.

---

## Entities not introduced

- **Model catalog** — Phase 6. `Model` stays a domain concept only.
- **Routing or fallback policy** — Phase 6.
- **Credential storage** — the credential is read from the environment at
  startup and held by the adapter. No new entity, and it is never logged or
  returned.
- **Per-call metrics** — Phase 12. No entity, since the substrate does not
  exist yet.
