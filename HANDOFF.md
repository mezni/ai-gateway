# AI Gateway - Session Handoff

This document is used to hand off work between development sessions. Update it after completing each phase.

## Current State

- **Date**: 2026-09-30
- **Phase**: Phase 0 - Project Foundation (Complete) / Phase 1 - Architecture Specification (Complete) / Phase 2 - HTTP Gateway Core (Complete) / Phase 3 - Domain Models and Validation (Complete) / Phase 4 - Provider Abstraction (Complete)
- **Branch**: main
- **Constitution Version**: 1.1.0

## Completed

- [x] Project documentation created (PRD, Architecture, API, Configuration, Providers, Security, Reliability, Plan)
- [x] README, CHANGELOG.md, and HANDOFF.md created
- [x] Directory structure established (docs/, specs/, docs/adr/)
- [x] Constitution created, ratified, and amended (Spec Kit requirements, v1.1.0)
- [x] Phase 0 project foundation: zero-dependency Rust crate (Edition 2024, toolchain pinned 1.98.1), lib/main entry point, quality-gate workflow, `specs/001-project-foundation` feature complete (18/18 tasks)
- [x] Phase 1 layered architecture (`specs/002-layered-architecture`, 16/16 tasks): lib-first module skeleton (`domain`/`application`/`api`/`infrastructure`/`config`, file-stem layout), provider-independent domain types (`ChatRequest`, `Message`, `MessageRole`, `ChatResponse`, `Usage`, `Model`, `Provider`), `AppState` composition root; all quality gates green
- [x] Phase 2 HTTP gateway core (`specs/003-http-gateway-core`, 48/48 tasks, requirements checklist 16/16): Axum 0.8 server exposing `GET /health`, `GET /ready`, and `POST /v1/chat/completions`; stateless deterministic local mock chat completion; flat `{"code","message"}` error contract across seven rows; lifecycle-driven readiness (Initializing → Ready → ShuttingDown → Stopped) and graceful shutdown under a single absolute 10-second deadline; all quality gates green (120 tests)
- [x] Phase 3 domain models and validation (`specs/004-domain-validation`, 51/51 tasks, requirements checklist 16/16): ordered nine-stage validation pipeline (`required-field` -> `control-range` -> `streaming`) in `src/application/chat.rs`; `temperature` (`0.0`-`2.0` inclusive) and `max_tokens` (`1`-`4096` inclusive) as optional domain fields; manual `MapAccess` deserializer refusing non-numbers, non-integers, explicit `null`, and duplicate controls while still ignoring unknown fields; 1 MiB inclusive whole-body limit (`1_048_576`) enforced after admission and before media-type handling with the new `413 payload_too_large` row; `stream: true` refused as the pipeline's final stage; all 20 catalogued rules, all nine precedence stages, the size boundary, 50-request concurrency isolation, and lifecycle behavior covered automatically; all quality gates green (203 tests)
- [x] Phase 4 provider abstraction (`specs/005-provider-abstraction`): `LlmProvider` trait with `ProviderFailure` enum (5 categories), `ProviderConfig` with env vars `AI_GATEWAY_PROVIDER` and `AI_GATEWAY_PROVIDER_TIMEOUT_MS` (deterministic default), `ProviderRegistry` auto-registering deterministic provider, `502 provider_unavailable` and `504 provider_timeout` error codes extending client-facing contract from 8 to 10, `provider` field in `ReadinessResponseDto` (FR-008), bounded provider calls with deadline enforcement (FR-011), provider selection wired in `main.rs` via `AppState::new_with_provider()`, test count grown to 245 across all test suites

## In Progress

None at this time.

## Next Steps

1. Start Phase 5 using Spec Kit (`/speckit.specify`) targeting `docs/plan.md` section 10 (Phase 5 - Reliability and Production Readiness)
2. Provider abstraction is complete; US1 and US2 validated; US3 extensibility ready
3. Phase 5: introduce Redis, reliability patterns, production deployment
3. Phase 3 deliberately left the 1 MiB limit, the control ranges, and the 14-row error contract fixed; do not make the limit configurable, add a `details` field, or add per-field content limits here
4. Before implementing any phase that needs authentication, request correlation, or provider errors (sections 22-28), decide whether the wrapped `{"error":{"type",...,"request_id"}}` shape in `docs/api.md` section 19 is adopted or dropped. Adopting it is a breaking change to the Phase 2 flat `{"code","message"}` contract and needs its own spec; the 14-row client-facing contract in `specs/004-domain-validation/contracts/http-api.md` covers 15 conditions across 8 codes and must stay byte-exact for the 14 currently reachable ones

## Known Issues / Blockers

None at this time. Authentication, provider routing, streaming, usage reporting, request IDs, and dependency-aware readiness are deliberately deferred to later phases.

## Important Context

- Spec-driven development via Spec Kit (constitution v1.1.0)
- Rust-first, incremental architecture; module boundary contract at `specs/002-layered-architecture/contracts/layout.md` (api → application → domain, infrastructure → domain)
- Phase 2 modules: `src/api/{server,health,chat,dto,error,middleware}.rs`, `src/application/{chat,lifecycle}.rs`, `src/domain/chat.rs`, `src/config/server.rs`; the api layer owns HTTP concerns, the application layer owns the lifecycle and the mock chat service, and the domain layer stays provider-independent
- Current chat scope is mock-only: no providers, no authentication or authorization, no streaming, no persistence, and no network egress
- Phase 3 validation contract is authoritative in `specs/004-domain-validation/contracts/validation-rules.md` (20 rule IDs, nine-stage precedence) and `contracts/http-api.md`; `specs/003-http-gateway-core/contracts/http-api.md` carries a supersession note so the two documents do not contradict each other
- `docs/api.md` is the *target* API, not a description of the running gateway. It now opens with a status banner that splits every section into implemented (5, 7, 8, 9, 10, 11, 13, 18, 21) versus planned (4, 6, 15, 16, 17, 19, 22-32). Sections 16, 18, 19, 20, and 40 carry inline warnings. When implementing a later phase, update that banner in the same change — it is the only thing stopping a reader from coding against the wrapped `{"error":{...}}` shape in section 19, which the gateway does not emit
- `ChatRequest` holds an `f64` (`temperature`) and therefore derives `PartialEq` without `Eq`; the same applies to `CompleteChatCommand` and `ChatCompletionRequestDto`
- `serde_json` appends a positional `at line N column M` suffix to custom deserializer errors; that suffix is discarded at the API boundary by `ApiError::from_json_rejection`, so it can never reach a client
- The size bound lives in `bound_chat_body` middleware rather than `DefaultBodyLimit`, because `Json` checks the media type before buffering and an oversized body with a wrong media type must return 413, not 415
- Configuration comes from the process environment only (`AI_GATEWAY_HOST`, `AI_GATEWAY_PORT`); no `.env` file is loaded automatically
- Quality gates: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo build`, `cargo build --release`, `cargo run --quiet`
