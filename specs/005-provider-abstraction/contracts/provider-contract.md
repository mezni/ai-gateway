# Contract: Provider Boundary

**Feature**: `specs/005-provider-abstraction`
**Date**: 2026-09-30
**Status**: Internal. Not a client-facing surface.
**Governs**: How the gateway reaches a provider, and what every adapter must satisfy.

This contract exists so that Phase 5 implements a hosted adapter against a
pre-agreed interface rather than inventing one. It is the internal seam
required by constitution principle IV (Provider Independence).

---

## 1. The provider contract

A single provider-agnostic contract, object-safe so providers can be held
heterogeneously.

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Produces one completion for an already-validated request.
    ///
    /// The request has already passed the full Phase 3 validation pipeline.
    /// An adapter MUST NOT re-validate it and MUST NOT assume the caller did
    /// not.
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderFailure>;

    /// The provider's stable identifier, for logging and readiness reporting.
    fn id(&self) -> &str;
}
```

### Why `async_trait` and not native `async fn`

RPITIT is not dyn-compatible. A trait whose method returns `impl Future`
cannot be stored as `Arc<dyn Trait>`, and provider selection requires exactly
that. This was verified by compiling both forms against the pinned toolchain
(Rust 1.98.1, Edition 2024); see [research.md](../research.md) D-002.

A hand-written boxed-future return type is a working alternative and is
recorded there, but it pushes `Box::pin` boilerplate onto every adapter.

### Rules

| Rule | Requirement |
|---|---|
| FR-001 | This is the only way the gateway reaches a provider. No adapter may be called directly by the API or routing layers. |
| FR-002 | `Send + Sync`, and usable concurrently through shared ownership. |
| FR-003 | The signature contains no provider-native type. |
| FR-004 | The signature's request and response types MUST NOT gain provider-specific fields. |
| FR-022 | A provider's native request and response formats are confined to its own adapter. |

---

## 2. Provider failure categories

The complete set. A caller branches on the category, never on a message.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFailure {
    Unreachable,
    Refused,
    DeadlineExceeded,
    UnusableResponse,
    InvalidResponse,
}
```

| Category | Means | Raised when | Client code |
|---|---|---|---|
| `Unreachable` | Could not be contacted | Connection refused, DNS, TLS, no route | `502 provider_unavailable` |
| `Refused` | Reached and declined | Provider rejected the request | `502 provider_unavailable` |
| `DeadlineExceeded` | Deadline elapsed | The bounded wait won | `504 provider_timeout` |
| `UnusableResponse` | Body not interpretable | Malformed, wrong shape, bad encoding | `500 internal_error` |
| `InvalidResponse` | Interpretable, no completion | Completion absent or empty | `500 internal_error` |

### Rules

- **FR-010**: exactly these five distinctions. A category not listed here does
  not exist.
- **FR-012**: the type MUST NOT carry provider status codes, provider message
  text, or credentials. It carries the category only. Provider detail is
  therefore unable to reach a client by construction rather than by filtering.
- **FR-013**: a failure is never reported as a success, and partial provider
  output is never reported as a success.
- No category maps to a validation code. A provider fault MUST NOT be reported
  as `invalid_request`.

### Choosing a category

| Situation | Category |
|---|---|
| DNS resolution failed | `Unreachable` |
| TCP connection refused | `Unreachable` |
| TLS handshake failed | `Unreachable` |
| Provider returned a rejection for this request | `Refused` |
| The configured deadline elapsed | `DeadlineExceeded` |
| Provider returned success but the body did not parse | `UnusableResponse` |
| Provider returned success, body parsed, no completion present | `InvalidResponse` |
| The adapter hit an internal bug while translating | `UnusableResponse` |

The last row is a deliberate choice: an adapter translation bug is indistinguishable
from an unusable provider response at the client boundary, and reporting it as
`500` keeps the honest message while the detail stays in gateway-side logs.

---

## 3. Adapter obligations

Every adapter MUST satisfy all of these. They are the constitution's LLM
Provider Integration standard expressed as a testable interface.

| Obligation | Requirement | Phase 4 status |
|---|---|---|
| Authentication | Reads its credential from the environment at startup; never hard-codes, logs, or returns it | Deferred to Phase 5. The deterministic adapter needs no credential, so the obligation is specified but exercised by a test that asserts no credential appears in any response or log. |
| Request transformation | Maps `ChatRequest` to its native format **inside the adapter only** | Required and exercised by the test double. |
| Response transformation | Maps its native response to `ChatResponse` **inside the adapter only** | Required and exercised by both adapters. |
| Provider errors | Classifies every failure into one of the five categories; never passes a raw provider error upward | Required and exercised by the test double, which can be made to fail in each category. |
| Timeouts | Does **not** implement its own timeout; the application layer bounds the call | Required, and the absence of adapter-side timeouts is itself asserted. |
| Usage information | Passes through what the provider reported; never fabricates counts | Required. `ChatResponse::usage` is already optional. |
| Provider-specific metadata | Confined to the adapter | Required. |

### Adding a provider

Per **FR-021**, adding a provider requires:

1. Implement `LlmProvider` in one new file under the infrastructure layer.
2. Register it in the registry.
3. Nothing else.

It MUST NOT require changes to request validation, to the client-facing
response, or to any other provider. SC-001 verifies this by adding a test-only
provider, using it, and removing it, with no other file edited.

---

## 4. Deadline enforcement

Enforced by the application layer, not by adapters.

```text
application layer
    |
    |-- wraps the provider future in a bounded wait
    |      using the startup-validated deadline
    |
    v
provider.chat(request)
```

| Rule | Requirement |
|---|---|
| FR-011 | Every provider call is bounded. A provider that never responds MUST NOT hold a client request open indefinitely. |
| FR-010 | When the bound elapses first, the result is `DeadlineExceeded`, never a hang and never a partial success. |

Because the bound is applied centrally, an adapter cannot forget it. This is
deliberate: if enforcement lived in each adapter, FR-011 would hold only for
adapters that happened to implement it.

---

## 5. Startup resolution

```text
AI_GATEWAY_PROVIDER          -> which provider serves requests
AI_GATEWAY_PROVIDER_TIMEOUT_MS -> the bounded call deadline
```

| Rule | Requirement |
|---|---|
| FR-005 | Selection happens once, at startup, from the environment. |
| FR-007 | An unknown or disabled provider fails startup with a message naming the requested value. The process MUST NOT start in a state where every request fails. |
| FR-008 | The resolved provider is discoverable through the existing readiness surface. |
| FR-028 | Environment variables only. No configuration file, no serialization format. |
| — | An unparsable or non-positive deadline fails startup. |

Selecting the default deterministic provider requires **no** environment
variables at all, so the gateway still runs with zero configuration and zero
network egress.

---

## 6. What this contract deliberately does not include

| Excluded | Phase | Reason |
|---|---|---|
| Live provider connectivity, credentials in use, native format mapping | 5 | FR-026 |
| Per-request provider selection, routing, fallback | 6 | Needs a routing policy |
| Retry policy | 10 | FR-013 forbids silent duplication; a bounded policy needs the reliability design |
| Streaming | 11 | Streaming is a different response shape entirely |
| Per-call metrics and traces | 12 | The observability substrate does not exist yet |

---

## 7. Invariants

These hold for every provider, every request, and are asserted by the suite.

1. No provider call is made for a request that failed Phase 3 validation, or
   whose body exceeded the size limit. An invalid request can never cause a
   billable upstream call.
2. Provider-native types never appear above the adapter boundary.
3. No provider status code, provider message, or credential appears in any
   client-visible byte.
4. The client-facing response is byte-identical for an equivalent completion,
   regardless of which provider produced it.
5. Identical input to the deterministic provider yields a byte-identical
   completion, so the gateway's own suite stays a reliable regression net.
6. No state is shared between requests. Concurrent requests to different
   providers cannot observe each other.
7. A provider failure never terminates the gateway, and health, readiness, and
   subsequent requests keep working afterwards.
