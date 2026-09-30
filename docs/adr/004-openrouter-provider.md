# ADR 004: OpenRouter Provider Integration

**Status**: Implemented  
**Date**: 2026-09-30  
**Author**: AI Gateway Project  
**Tags**: provider, openrouter, phase-5

## Context

The AI Gateway previously had no LLM provider integration beyond the built-in deterministic provider. This ADR addresses the need to connect the gateway to an LLM provider via OpenRouter, as specified in `specs/006-openrouter-integration` and `docs/plan.md` Phase 5.

## Decision Drivers

- **FR-001**: The gateway MUST support connecting to OpenRouter as an LLM provider.
- **FR-002**: The gateway MUST send chat completion requests to OpenRouter with proper authentication headers.
- **FR-003**: The gateway MUST map internal request models to OpenRouter API format.
- **FR-004**: The gateway MUST map OpenRouter API responses to internal response models.
- **FR-005**: The gateway MUST handle external API failures gracefully without crashing, mapping all errors to the gateway's flat two-key contract `{"code","message"}`.
- **Constitution Principle IV (Provider Independence)**: Provider-specific logic MUST be isolated inside provider adapters.
- **Constitution Principle V (Explicit Boundaries and Layered Architecture)**: Provider implementations MUST belong in `infrastructure`, not in `application` or `domain`.
- **Constitution Principle IX (Testability and Correctness)**: Provider interactions MUST be mockable or replaceable with deterministic test implementations. External LLM providers MUST NOT be required for ordinary unit tests.
- **005 Provider Contract FR-028**: Environment variables only. No configuration file, no serialization format.

## Decision: Env-Var-Driven Provider Configuration (Not YAML)

**Decision**: Provider configuration is materialized from environment variables at startup. The `config/gateway.yaml` file is an operator-facing reference document, **not** read by the gateway.

**Rejected**: Adding a YAML config loader to the `config` layer (would violate 005 FR-028 and 002 layout rule 5: config depends only on `std::env` + `thiserror`).

**Implementation**: 
- `OpenRouterConfig::from_env()` reads `AI_GATEWAY_OPENROUTER_ENABLED`, `AI_GATEWAY_OPENROUTER_BASE_URL`, `AI_GATEWAY_OPENROUTER_API_KEY_ENV`.
- `OpenRouterProvider::from_config()` constructs the adapter from the resolved config.
- The API key is read from the env var named by `api_key_env` at startup; a missing/empty key does not fail startup — the provider refuses requests locally.

**File**: `src/config/openrouter.rs`  
**Error type**: `OpenRouterConfigError` / `OpenRouterProviderError`

## Decision: Provider Adapter in Infrastructure Layer

**Decision**: The `OpenRouterProvider` adapter lives in `src/infrastructure/providers/openrouter.rs`, implementing the `LlmProvider` trait from the application layer.

**Rejected**: Placing the adapter in `application` (would require `application` to import infrastructure types, violating layout rule 4).

**Implementation**: 
- `src/infrastructure/providers/openrouter.rs` — one file, self-contained.
- `src/infrastructure/providers/mod.rs` — registers `mod openrouter` + re-exports.
- `src/main.rs` — wires `ProviderRegistry` + `OpenRouterProvider::from_config()` + `AppState::new_with_provider_and_config()`.

**File**: `src/infrastructure/providers/openrouter.rs`  
**Exports**: `OpenRouterProvider`, `OPENROUTER_ID`, `OpenRouterProviderError`

## Decision: Request/Response Mapping Inside Adapter

**Decision**: The adapter translates `ChatRequest` <-> OpenRouter JSON and back, confined entirely within the adapter boundary (FR-022).

**Rejected**: Any shared model or DTO crossing the adapter boundary.

**Mapping details**:
- **Request**: `ChatRequest` → OpenRouter `{"model", "messages", "temperature" (opt), "max_tokens" (opt)}`. `stream` absent (validation rejects it).
- **Response**: OpenRouter 200 with `choices[0].message.content` → `ChatResponse.content`. Usage derived from `prompt_tokens` + `completion_tokens`. 200 with `error` object → `ProviderFailure::Refused`. Malformed/empty → `ProviderFailure::InvalidResponse` / `UnusableResponse`.
- **Error classification**: transport error → `Unreachable` / `DeadlineExceeded`; non-2xx HTTP → `Refused`; unparseable body → `UnusableResponse`; missing completion → `InvalidResponse`.

**File**: `src/infrastructure/providers/openrouter.rs`  
**Test coverage**: 30+ unit tests + 10+ integration tests with local fake OpenRouter server.

## Decision: Error Mapping to Frozen 10-Row Contract

**Decision**: Provider failures map to the existing 10-row client-facing error contract (`ApiError`): `provider_unavailable` (502), `provider_timeout` (504), `invalid_request` (400), etc.

**Rejected**: Adding `authentication_error`, `rate_limit_error`, `not_found_error` rows — would break the frozen Phase 3/4 contract and fail the `all_ten_statuses_are_distinct` and `every_documented_row_matches_status_code_and_code` tests.

**Mapping**:
- `ProviderFailure::Refused` → `ApiError::ProviderUnavailable` (502), `code: "provider_unavailable"`
- `ProviderFailure::DeadlineExceeded` → `ApiError::ProviderTimeout` (504), `code: "provider_timeout"`
- `ProviderFailure::Unreachable` → `ApiError::ProviderUnavailable` (502)
- `ProviderFailure::UnusableResponse` → `ApiError::InternalError` (500)
- `ProviderFailure::InvalidResponse` → `ApiError::InternalError` (500)

**File**: `src/api/error.rs` — no changes needed (mapping already covers these categories).

## Decision: Telemetry (Minimal, No New Dependencies)

**Decision**: Per 005 contract §6, per-call metrics and traces are deferred to Phase 12. 006 implements a lightweight, dependency-free telemetry signal.

**Implementation**:
- `ChatTelemetry` in `src/application/telemetry.rs` — `Arc<AtomicU64>` counter + `record_chat()` method.
- `ChatRequestSignal` — plain struct with `provider`, `model`, `latency_ms`, `status`, `error_type`, `attempt`, `request_count`.
- Rendering in `src/api/telemetry.rs` — `chat_signal_line()` + `emit_chat_signal()` → `println!("{}")` to stdout.
- Fields never contain prompts, message bodies, or credentials.

**File**: `src/application/telemetry.rs`, `src/api/telemetry.rs`

## Decision: Integration Tests with Local Fake Server

**Decision**: Contract and integration tests use a local `axum` test router serving as a fake OpenRouter API, avoiding external network calls.

**Implementation**: 
- `tests/provider_abstraction.rs` — adapted from existing test file.
- New test patterns in `src/infrastructure/providers/openrouter.rs` — `#[cfg(test)]` module with unit tests for mapping, classification, and credential redaction.
- Integration-style tests in the same file — `start_fake()`, `fake_handler()` — spin up a local axum router on a random port, point the provider at it, and assert request/response shape, auth header, and error paths.

**File**: `src/infrastructure/providers/openrouter.rs` (test module)

## Decision: No Provider-Specific Types Above Adapter Boundary

**Decision**: The gateway's internal `ChatRequest`, `ChatResponse`, `ProviderFailure` types never contain provider-native fields. All provider-specific types are confined to the adapter.

**Rejected**: Any provider type leaking above the adapter.

**Verification**: `the_signal_cannot_carry_a_prompt_or_a_credential` test; `a_signal_cannot_carry_a_prompt_or_a_credential` test; `the_renderer_has_no_field_of_its_own_beyond_the_signal` test.

## Decision: Roles Deferred to Phase 7/8

**Decision**: "Support two user roles: user (chat requestors) and admin (operational management)" is not part of 006's scope. The provider's `enabled` flag and the gateway's `AI_GATEWAY_PROVIDER` configuration serve the operational management use case. Role-based authentication is a Phase 7 concern per HANDOFF.md.

**Rejected**: Implementing a role system within the provider config for this phase.

## Related ADRs
- ADR 001: Rust-first engineering (constitution)
- ADR 002: Explicit boundaries and layered architecture (constitution)
- ADR 003: Provider abstraction (005 provider-abstraction)

## Coordination
- Related to `specs/006-openrouter-integration` feature specification.
- Related to `docs/plan.md` Phase 5 — OpenRouter Integration.
- Related to `specs/005-provider-abstraction` — provider contract and registry.
- Related to `specs/002-layered-architecture` — layout and dependency rules.

## References
- `specs/006-openrouter-integration/spec.md` — feature specification.
- `docs/plan.md` §10 — Phase 5 — OpenRouter Integration.
- `specs/005-provider-abstraction/contracts/provider-contract.md` — provider contract.
- `specs/005-provider-abstraction/tasks.md` — Phase 4 tasks.
- `src/config/openrouter.rs` — configuration.
- `src/infrastructure/providers/openrouter.rs` — adapter implementation.
- `src/application/telemetry.rs` — telemetry state.
- `src/api/telemetry.rs` — signal rendering.
- `src/main.rs` — composition root wiring.

---

**Previous**: N/A (first OpenRouter integration ADR)  
**Superseded by**: N/A (new integration)  
**Related**: ADR 001–003, constitution.md  
**Location**: `docs/adr/004-openrouter-provider.md`