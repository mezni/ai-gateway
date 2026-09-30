# Implementation Plan: Provider Abstraction

**Branch**: `005-provider-abstraction` | **Date**: 2026-09-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-provider-abstraction/spec.md`

## Summary

Introduce a provider boundary between the gateway's request path and any
source of chat completions. A single provider-agnostic contract, awaited
concurrently and expressed only in the gateway's own domain types, replaces
today's synchronous and infallible `MockChatCompletionService::complete`. Every
provider's native request and response format is confined to that provider's
own adapter.

Scope is the boundary, startup-time provider selection through environment
variables, failure categorization, and one built-in deterministic provider. A
second provider exists only as a test double, registered in tests, so the
boundary can be proven without a live service. Live connectivity to a hosted
model service is Phase 5.

Two client-facing error codes are added, `502 provider_unavailable` and `504
provider_timeout`, extending the frozen eight-code contract to ten. Everything
else the gateway already guarantees is unchanged and must stay that way.

## Technical Context

**Language/Version**: Rust 1.98.1, pinned in `rust-toolchain.toml` (Edition 2024, `minimal` profile with `clippy` and `rustfmt`)

**Primary Dependencies**: `axum` 0.8 (HTTP), `tokio` 1 (async runtime, `rt-multi-thread`/`sync`/`time`/`net`), `serde` + `serde_json` (JSON at the transport edge only), `thiserror` 2 (structured errors), `anyhow` 1 (contextual propagation), `http-body-util` 0.1

**Storage**: N/A. No persistence in this phase; `docs/plan.md` defers PostgreSQL to Phase 15 and Redis to Phase 16.

**Testing**: `cargo test --all-targets`, using `tower::ServiceExt::oneshot` for in-process router assertions and `tokio` for concurrency. No external provider is contacted in any test.

**Target Platform**: Linux server, `cargo run` as a local process bound to `AI_GATEWAY_HOST`:`AI_GATEWAY_PORT`

**Project Type**: web-service (HTTP gateway), lib-first so `src/lib.rs` is importable by tests and `src/main.rs` is a thin binary

**Performance Goals**: A provider call MUST be fully bounded by a configured deadline; no client request may remain outstanding beyond it. No per-request allocation of provider state; providers are resolved once at startup and shared.

**Constraints**: Zero network egress in the default configuration. No new route. No new configuration file. No `details` field, wrapper object, or per-category list in any error body. Providers MUST be `Send + Sync` and usable concurrently through shared ownership.

**Scale/Scope**: 50 concurrent requests across providers is the stated verification load (SC-005). One built-in provider ships. Client-facing error codes go from 8 to 10.

### New dependency decision

`async-trait` **is** added, as a single proc-macro dependency. This corrects an
earlier draft of this plan that assumed native `async fn` in traits would
suffice. That assumption is wrong: RPITIT is not dyn-compatible, so
`Arc<dyn LlmProvider>` would not compile, and provider selection requires
exactly that. Both the proc-macro crate and a hand-written boxed-future
alternative were compiled and verified against the pinned toolchain; the
rationale and the rejected alternative are recorded in
[research.md](research.md) D-002.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-research gate

| Principle / Standard | Requirement | Status | Evidence or justification |
|---|---|---|---|
| I. Specification-Driven | All functionality begins with a written spec | PASS | `specs/005-provider-abstraction/spec.md`, checklist 16/16 |
| II. Rust-First | Rust; idiomatic; no `unsafe`; prefer compile-time guarantees | PASS | Rust 1.98.1. Native `async fn` in traits avoids a proc-macro dep. No `unsafe` introduced; this code path is entirely safe. |
| III. Incremental Architecture | No infrastructure before a spec requires it; each phase leaves a runnable, testable system | PASS | FR-026 excludes live providers. Default config performs zero network egress, so the gateway remains fully runnable and testable. |
| IV. Provider Independence | Providers reached only via explicit abstraction; native formats isolated in adapters; adding a provider must not change unrelated components | PASS | This feature is the direct implementation of IV. Enforced by FR-003, FR-004, FR-021, FR-022, and verified by SC-001, SC-008. |
| V. Explicit Boundaries | Business logic not embedded in handlers; provider logic must not leak into routing or API | PASS | Provider trait and error type live in the application layer; implementations in `infrastructure`. The API layer keeps translating application errors to its own transport error, unchanged. Satisfies `layout.md` rules 4 and 8. |
| VI. Reliability and Failure Isolation | External calls unreliable; MUST handle timeouts, connection failures, provider errors, invalid responses; bounded retries; graceful shutdown | PASS | FR-010 (five failure categories), FR-011 (bounded deadline), FR-013 (no partial success), FR-018 (containment), FR-019 (graceful shutdown). No retry or fallback is introduced, which the principle permits: "Retries MUST NOT be introduced where they can create unsafe request duplication." |
| VII. Security and Privacy | Credentials never hard-coded, logged, or returned | PASS | FR-023, reinforced by the LLM Provider Integration standard. API-key handling verified by an explicit no-leak test. |
| VIII. Observability | Every significant component observable | PASS **with a recorded gap** | Selection is discoverable through the existing readiness surface (FR-008) and the selected provider is named in the startup-refusal message. Per-call metrics and traces are Phase 12 and are listed out of scope. No new dependency and no new route is introduced here. |
| IX. Testability | Every significant component automatically tested; external providers MUST NOT be required for unit tests; provider interactions mockable or replaceable | PASS | FR-006a mandates a test double as the second provider precisely to satisfy this. No test contacts a network. |
| X. Performance | Async I/O for network-bound work; no blocking on runtime workers; avoid unnecessary cloning | PASS | `async fn` on the trait; providers behind `Arc` and resolved once at startup. Zero egress in the default path. |
| Std: Error Handling | Explicit classified errors via `thiserror`; must not expose secrets or provider detail; consistent API error format | PASS | New `ProviderError` uses `thiserror`. FR-012 forbids provider text, status codes, and credentials in responses. The two new codes keep the flat two-field body. |
| Std: Configuration | Externalized; distinguish application from provider configuration; fail fast on invalid critical config; validate at startup | PASS | FR-005, FR-007, FR-028. Provider selection is provider configuration and is read from the environment, distinct from `AI_GATEWAY_HOST`/`AI_GATEWAY_PORT`. Unknown provider fails startup, which the standard requires. |
| Std: LLM Provider Integration | Adapters MUST handle auth, request/response transformation, provider errors, timeouts, usage | PASS **with a scoped deferral** | A full hosted adapter is Phase 5 by FR-026. This phase specifies the seam each adapter must satisfy and proves it with the deterministic adapter plus a test double. Recorded in Complexity Tracking. |
| Std: Documentation | Documentation updated where necessary | PASS | The Phase 3 status banner in `docs/api.md` is updated in this feature, because the error-code count changes from 8 to 10. |

**Gate result: PASS.** No unjustified violations. One deferral and one recorded
gap are justified below and in Complexity Tracking.

### Post-design re-check

Re-evaluated after Phase 1. The design added no new layer, no new route, no new
configuration format, and no new external dependency. `research.md` D-001 through
D-007 were all resolved without deviating from a principle. Principle VIII's gap
is unchanged and remains scoped to Phase 12. **Result: PASS.**

## Project Structure

### Documentation (this feature)

```text
specs/005-provider-abstraction/
├── plan.md              # This file (/speckit.plan command output)
├── spec.md              # Feature specification (input, unchanged by this command)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
└── checklists/
    └── requirements.md  # Spec quality checklist, 16/16
```

### Source Code (repository root)

```text
src/
├── lib.rs
├── main.rs
├── config/
│   ├── mod.rs
│   └── server.rs
├── domain/
│   ├── mod.rs
│   ├── chat.rs          # ChatRequest, Message, MessageRole, ChatResponse, Usage
│   └── catalog.rs       # Model, Provider (concept only)
├── application/
│   ├── mod.rs           # AppState composition root
│   ├── chat.rs          # + LlmProvider trait, ProviderError, MockLlmProvider
│   └── lifecycle.rs
├── infrastructure/
│   ├── mod.rs           # was an empty placeholder; now the provider home
│   └── providers/
│       ├── mod.rs        # + ProviderRegistry, resolve from config
│       └── deterministic.rs  # + DeterministicProvider adapter
├── api/
│   ├── mod.rs
│   ├── server.rs        # unchanged route surface
│   ├── health.rs
│   ├── chat.rs          # + maps ProviderError to 502/504
│   ├── dto.rs
│   ├── error.rs         # + PayloadTooLarge's siblings: ProviderUnavailable, ProviderTimeout
│   └── middleware.rs
└── ...

tests/
├── http_api.rs          # + provider selection, 502/504, isolation
├── provider_abstraction.rs  # + NEW: boundary and adapter tests
└── server_lifecycle.rs
```

**Structure Decision**: The existing single-crate, five-layer, file-stem layout
from `specs/002-layered-architecture` is retained. Two consequences are
deliberate and follow `layout.md` rule 8, which already placed provider
implementations in the infrastructure layer while that layer was still an empty
placeholder:

- The provider **contract** and its **error type** live in
  `src/application/chat.rs`, beside the existing service, because
  `layout.md` rule 4 forbids `application` from importing `infrastructure`.
  Putting the trait in `infrastructure` would make it unreachable from the
  application layer that must call it.
- Provider **implementations** live under `src/infrastructure/providers/`.
  `infrastructure.rs` stops being an empty placeholder, which
  `layout.md` rule 9 anticipated for exactly this phase.

The application layer receives providers through `AppState`, preserving
dependency injection: the API layer never names a provider.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| LLM Provider Integration standard requires every adapter to handle authentication, request/response transformation, provider errors, timeouts, and usage. This phase ships no hosted adapter, so the standard is exercised only by the deterministic adapter and a test double. | `docs/plan.md` section 9 and section 10 assign live OpenRouter integration, including auth and native format mapping, to Phase 5. The user resolved this explicitly by accepting FR-026 as written. | Implementing a hosted adapter now would require live network calls and an API key to pass any meaningful test, which constitution principle IX forbids for ordinary unit tests, and would pull an entire phase's scope forward. The standard's requirements are instead encoded as the seam every adapter must satisfy, recorded in [contracts/provider-contract.md](contracts/provider-contract.md), so Phase 5 implements against a pre-agreed interface rather than inventing one. |
| Principle VIII observability is only partially satisfied: selection is discoverable, but there are no per-call metrics or traces for provider calls. | Observability as a first-class capability is Phase 12 (`docs/plan.md` section 17), which is where metrics, tracing, and logging infrastructure are introduced. Adding instrumentation now would require the logging and metrics substrate that does not exist yet. | Adding a partial ad-hoc instrumentation scheme now would create a second thing to migrate in Phase 12 and would violate the project's incrementalism. The gap is recorded in the pre-research gate above so it is visible rather than forgotten. |
