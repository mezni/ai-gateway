# Implementation Plan: HTTP Gateway Core

**Branch**: `003-http-gateway-core` | **Date**: 2026-09-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-http-gateway-core/spec.md`

## Summary

Turn the one-shot Rust crate into a long-running local HTTP service with liveness,
readiness, and deterministic mock chat-completion endpoints. The implementation
uses Axum and Tokio behind the existing layered architecture, keeps HTTP DTOs in
the API layer, maps them to application-owned commands before domain conversion,
tracks chat admission in the application lifecycle, and drains in-flight work
without making liveness or readiness unreachable. Configuration, API errors, and
startup/shutdown behavior remain explicit and provider-free.

## Technical Context

**Language/Version**: Rust 1.98.1, Edition 2024

**Primary Dependencies**: Axum 0.8, Tokio 1.x, Serde 1.x, serde_json 1.x,
thiserror 2.x, and anyhow 1.x; Tower 0.5 with `util` for router tests

**Storage**: N/A; all state is process-local and non-durable

**Testing**: `cargo test`; unit tests, in-process Axum router tests with
`ServiceExt::oneshot`, and bound-port lifecycle tests on `127.0.0.1`

**Target Platform**: Local Linux server with Ctrl-C and SIGTERM support

**Project Type**: Single Rust library plus thin binary web service

**Performance Goals**: At least 95% of 100 health and 100 readiness requests
complete within 250 ms; repeated mock chat requests remain deterministic

**Constraints**: No provider, credential, database, cache, persistence,
authentication, routing, or streaming implementation; default listener
`127.0.0.1:3000`; at most 10 seconds for shutdown after the signal

**Scale/Scope**: One local gateway process, three endpoints, one deterministic
mock completion, no durable data

## Constitution Check

**Initial gate**: PASS. The active specification is complete, no technical
unknowns require user clarification, and the proposed work introduces only the
infrastructure required by Phase 2.

**Post-design re-check**: PASS.

| Principle / Gate | Status | Design evidence |
|------------------|--------|-----------------|
| Specification-driven development | PASS | Requirements map to this plan, the data model, contracts, and quickstart; `tasks.md` remains a later phase. |
| Rust-first engineering | PASS | Rust 1.98.1, safe synchronization, explicit enums/errors, no planned `unsafe`. |
| Incremental architecture | PASS | Adds only HTTP serving, lifecycle state, minimal validation, and a mock; provider/database infrastructure stays deferred. |
| Provider independence | PASS | No provider adapter or provider-specific type is introduced. |
| Explicit boundaries | PASS | Domain remains transport-free; API owns DTOs/routes/errors and depends on application/config; application commands own request validation and domain mapping; config owns environment parsing; `main.rs` remains a composition root. |
| Reliability and failure isolation | PASS | Startup fails before readiness, request errors are normalized, client cancellation releases admission, and shutdown drains within the 10-second bound. |
| Security and privacy | PASS | Endpoints are explicitly local/unauthenticated for this phase, credentials are not read, sensitive diagnostics are not serialized, and the API is not production-hardened. |
| Observability | PASS (phase scope) | Health/readiness, startup failures, and clean shutdown are observable; production telemetry is explicitly deferred by the specification. |
| Testability and correctness | PASS | Domain, lifecycle, config, middleware, all API contracts, failure paths, concurrency, and bound-port shutdown receive automated coverage. |
| Performance and resource discipline | PASS | Async I/O, bounded buffering, no blocking work on runtime workers, no external calls, and measured endpoint targets. |
| Documentation and governance | PASS | User quickstart, API/configuration/testing docs, environment contract, and quality-gate changes are planned. |

**Complexity exceptions**: None. No constitution violation is accepted or
deferred to implementation.

### Success-Criteria Verification

| Criterion | Planned verification |
|-----------|----------------------|
| SC-001 | Follow `quickstart.md` from a clean build and complete all endpoint/configuration flows in under 10 minutes. |
| SC-002 | Router/integration tests cover every endpoint, primary failure path, and all seven exact error rows without an external service. |
| SC-003 | Run 100 health and 100 readiness requests, assert every status/body, and measure that at least 95 per group complete within 250 ms. |
| SC-004 | Repeat the canonical chat request 100 times and assert exact model/content/choice plus absence of `usage`. |
| SC-005 | Exercise all seven error rows and assert exact status, code, message, field set, and absence of sensitive diagnostics. |
| SC-006 | Repeat the state matrix 20 times: router-level Initializing, bound-port Ready, and injected ShuttingDown; assert the exact liveness/readiness pairs. |
| SC-007 | Hold a real admitted HTTP request with a partial body during injected shutdown, verify probes and new-request rejection, then verify drain or the short test deadline and socket release. |
| SC-008 | Run the primary journey with no provider environment or external dependency and prove the mock path is local. |

## Project Structure

### Documentation (this feature)

```text
specs/003-http-gateway-core/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── spec.md
├── checklists/
│   └── requirements.md
└── contracts/
    ├── http-api.md
    └── runtime-configuration.md
```

### Source Code (repository root)

```text
src/
├── api.rs
├── api/
│   ├── chat.rs
│   ├── dto.rs
│   ├── error.rs
│   ├── health.rs
│   ├── middleware.rs
│   └── server.rs
├── application.rs
├── application/
│   ├── chat.rs
│   └── lifecycle.rs
├── config.rs
├── config/
│   └── server.rs
├── domain.rs
├── domain/
│   ├── catalog.rs
│   └── chat.rs
├── infrastructure.rs
├── lib.rs
└── main.rs
tests/
├── http_api.rs
└── server_lifecycle.rs
```

Existing source, dependency, documentation, and contract files updated with the
implemented behavior:

```text
Cargo.toml
Cargo.lock
src/lib.rs
src/main.rs
src/api.rs
src/application.rs
src/config.rs
src/domain.rs
src/domain/chat.rs
src/infrastructure.rs
.env.example
README.md
CHANGELOG.md
HANDOFF.md
docs/api.md
docs/configuration.md
docs/plan.md
docs/testing.md
specs/001-project-foundation/contracts/environment.md
specs/001-project-foundation/contracts/quality-gates.md
specs/002-layered-architecture/contracts/layout.md
specs/002-layered-architecture/data-model.md
```

**Structure Decision**: Keep the existing single-crate, file-stem module
convention. `main.rs` loads validated configuration, constructs the application
state and mock service, and delegates server execution. The API layer owns Axum
transport DTOs/routes/errors and maps them to application commands; the
application layer owns minimum request validation, domain mapping, lifecycle,
and orchestration; the config layer owns environment parsing; and the domain
remains independent of HTTP and serialization frameworks.

## Complexity Tracking

No constitution violations or undocumented complexity exceptions.

