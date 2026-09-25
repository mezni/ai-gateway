# Tasks: HTTP Gateway Core

**Input**: Design documents from `/specs/003-http-gateway-core/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/http-api.md`, `contracts/runtime-configuration.md`, `quickstart.md`

**Tests**: Required by FR-019 and SC-002. Write story tests first and confirm they fail for the intended missing behavior before implementation.

**Organization**: Tasks are grouped by user story so each story can be implemented, tested, and delivered independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other `[P]` tasks because it touches different files and has no unfinished dependency.
- **[Story]**: Maps the task to `US1`, `US2`, `US3`, or `US4`.
- Every task names the exact file or files it changes or validates.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the selected Rust HTTP stack and establish the file-stem module layout without changing current behavior.

- [X] T001 Add Axum 0.8, Serde 1.x with `derive`, serde_json 1.x, thiserror 2.x, anyhow 1.x, and Tokio 1.x with `macros`, `rt-multi-thread`, `net`, `signal`, `sync`, and `time` features to `Cargo.toml`, add Tower 0.5 with the `util` feature as a development dependency, and refresh `Cargo.lock`.
- [X] T002 [P] Create the API file-stem modules `src/api/chat.rs`, `src/api/dto.rs`, `src/api/error.rs`, `src/api/health.rs`, `src/api/middleware.rs`, and `src/api/server.rs`, and expose the expanded module tree through `src/lib.rs` and `src/api.rs` while preserving `app_version()`.
- [X] T003 [P] Create the application file-stem modules `src/application/chat.rs` and `src/application/lifecycle.rs`, and wire them through `src/application.rs` without adding API dependencies.
- [X] T004 [P] Create the configuration file-stem module `src/config/server.rs` and expose it through `src/config.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement the shared configuration, lifecycle, domain, DTO, error, state, and admission primitives required by all user stories.

**Critical**: No user story work can begin until this phase is complete.

- [X] T005 [P] Add failing unit tests for default values, valid IPv4/IPv6 overrides, empty and malformed host values, zero and malformed port values, and deterministic parsing without mutating the process environment in `src/config/server.rs`.
- [X] T006 Implement `ServerConfig` parsing from `AI_GATEWAY_HOST` and `AI_GATEWAY_PORT` in `src/config/server.rs`, using `127.0.0.1:3000` defaults, rejecting present invalid values, and document the two non-secret variables in `.env.example`.
- [X] T007 [P] Add failing lifecycle tests for `Initializing`, `Ready`, `ShuttingDown`, and `Stopped`, forward-only/idempotent transitions, ready-only admission, zero in-flight notification, and cancellation-safe guard drop in `src/application/lifecycle.rs`.
- [X] T008 Implement `GatewayPhase`, atomic lifecycle state, in-flight accounting, watch-based zero notification, and `ChatAdmissionGuard` in `src/application/lifecycle.rs`, with admission rechecked around the increment so shutdown cannot race a newly admitted chat request.
- [X] T009 [P] Add failing domain regression tests proving `ChatResponse::new` preserves measured usage and a no-usage constructor represents unavailable usage as `None` in `src/domain/chat.rs`.
- [X] T010 Change `ChatResponse.usage` to `Option<Usage>` in `src/domain/chat.rs`, wrap supplied values from the existing constructor in `Some`, and add a constructor for responses without measured usage.
- [X] T011 [P] Add failing serialization tests for exact `HealthResponseDto`, `ReadinessResponseDto`, and two-field `ApiErrorDto` field sets in `src/api/dto.rs`.
- [X] T012 Implement the shared operational-status and error DTOs in `src/api/dto.rs`, serializing only the documented fields and no diagnostics, details, or usage.
- [X] T013 Integrate `Arc<GatewayLifecycle>` into `AppState` in `src/application.rs`, preserve `AppState::new(version)` and the banner test, and add regression coverage that a new state starts in `Initializing` and is not ready.
- [X] T014 Add failing unit tests for status, code, exact fixed message, JSON field set, and `Content-Type` conversion for all seven API error variants in `src/api/error.rs`.
- [X] T015 Implement the centralized `ApiError` and `IntoResponse` conversion in `src/api/error.rs` for `invalid_request`, `unsupported_feature`, `unsupported_media_type`, `method_not_allowed`, `not_found`, `not_ready`, and `internal_error`, serializing only `ApiErrorDto`.
- [X] T016 Add failing middleware tests for Ready admission, Initializing/ShuttingDown/Stopped rejection, cancellation-safe count release, and in-flight notification in `src/api/middleware.rs`.
- [X] T017 Implement `ChatAdmissionMiddleware` and its RAII guard creation in `src/api/middleware.rs`, returning the centralized 503 `not_ready` response for every non-Ready phase.

**Checkpoint**: Run the foundational unit tests. Configuration, lifecycle, domain usage, DTOs, API errors, and chat admission are ready for story implementation.

---

## Phase 3: User Story 1 - Confirm Gateway Liveness (Priority: P1, MVP)

**Goal**: Run the gateway as a local long-lived HTTP service whose unauthenticated `GET /health` endpoint reports `{"status":"ok"}` without external dependencies.

**Independent Test**: Start the gateway with no provider or other service configuration, call `GET /health`, and verify HTTP 200, JSON content type, exact status body, concurrency isolation, and a process that remains running until signaled.

### Tests for User Story 1

> Write T018 first and confirm it fails because the liveness router is not implemented.

- [X] T018 [US1] Add failing liveness contract/concurrency tests for exact status, content type, body, external-service independence, and isolation in `tests/http_api.rs`, plus a bound-listener startup test proving the process remains available until its injected shutdown signal in `tests/server_lifecycle.rs`.

### Implementation for User Story 1

- [X] T019 [US1] Implement the stateless liveness handler in `src/api/health.rs`, returning HTTP 200 with `HealthResponseDto { status: "ok" }` in every externally reachable lifecycle phase.
- [X] T020 [US1] Build the Axum router factory in `src/api/server.rs`, register `GET /health` with `AppState`, and expose a reusable router for tests and the binary.
- [X] T021 [US1] Implement the Tokio TCP listener and server runner in `src/api/server.rs`, bind the configured address before the Ready transition, serve the router asynchronously, and expose a shutdown-future seam for later graceful coordination.
- [X] T022 [US1] Replace the one-shot banner binary in `src/main.rs` with synchronous configuration loading, Initializing state construction, server execution, and a nonzero contextual exit on startup failure.

**Checkpoint**: T018 passes and the gateway remains alive after serving `GET /health`; this is the first deployable MVP slice.

---

## Phase 4: User Story 2 - Determine Traffic Readiness (Priority: P1)

**Goal**: Expose `GET /ready` with distinct Ready and not-ready responses, transition only after bind succeeds, and preserve operational probes while draining chat traffic during shutdown.

**Independent Test**: Start on an ephemeral loopback port, verify `ok`/`ready`, drive Initializing and ShuttingDown router states, inject a shutdown signal while holding an admitted request, and verify the documented state pairs, 503 chat rejection, drain/deadline behavior, and socket cleanup.

### Tests for User Story 2

> Write T023 and T024 first and confirm they fail for the missing readiness and shutdown behavior.

- [X] T023 [US2] Add failing router tests in `tests/http_api.rs` for 20 repetitions of the Initializing `ok`/`not_ready`, Ready `ok`/`ready`, and ShuttingDown `ok`/`not_ready` state pairs.
- [X] T024 [P] [US2] Add failing bound-port lifecycle tests in `tests/server_lifecycle.rs` for successful ephemeral binding, occupied-port startup failure with address/port diagnostics, 100 real requests to each health and readiness endpoint with every status/body correct and at least 95 per group completing within 250 ms, SIGTERM-equivalent injected shutdown, a held admitted request plus client-disconnect guard release on a test-only handler mounted at the production chat path, a short forced-deadline case, 503 rejection of new chat admission, server completion, and listening-socket release.

### Implementation for User Story 2

- [X] T025 [US2] Implement the readiness handler in `src/api/health.rs`, returning HTTP 200 `ready` only in GatewayPhase::Ready and HTTP 503 `not_ready` in every other externally observable phase.
- [X] T026 [US2] Register `GET /ready` and complete bind-before-Ready startup in `src/api/server.rs`, then keep the listener serving health/readiness after the first Ctrl-C/SIGTERM, reject new chat through `ChatAdmissionMiddleware`, wait for admitted work to reach zero, trigger Axum graceful shutdown, and enforce one absolute 10-second deadline from the original signal.

**Checkpoint**: T023 and T024 pass; liveness and readiness remain distinguishable throughout startup, Ready operation, and shutdown.

---

## Phase 5: User Story 3 - Exercise a Mock Chat Completion (Priority: P1)

**Goal**: Accept a minimal OpenAI-compatible chat request and return one deterministic assistant choice locally, with no provider lookup, credentials, external call, or invented usage.

**Independent Test**: Send the canonical request through the running gateway with no provider configuration and verify the exact response envelope, requested model, assistant content, absence of usage/timestamp fields, deterministic repetition, and concurrent isolation.

### Tests for User Story 3

> Write T027 and T028 first and confirm they fail because the application use case and chat route are not implemented.

- [X] T027 [US3] Add failing application unit tests in `src/application/chat.rs` for minimum model/message/role/content validation, all supported roles, exact string preservation, whitespace acceptance, unsupported roles, opaque validated commands, and the fixed provider-free completion result.
- [X] T028 [US3] Add failing API contract tests in `tests/http_api.rs` for the canonical completion, all accepted roles, unknown models, exact single-choice envelope, no `usage` or `created`, stream rejection, invalid minimum fields, ignored future fields, 100 canonical repetitions that all return HTTP 200 with an identical parsed response envelope, and two concurrent requests with isolated model/content results.

### Implementation for User Story 3

- [X] T029 [US3] Implement `CompleteChatCommand`, `IncomingMessage`, `ValidatedChat`, `ChatCompletion`, and stateless `MockChatCompletionService` in `src/application/chat.rs`, with application-owned validation, ordered mapping to `domain::ChatRequest`, fixed mock content, and `ChatResponse` usage set to `None`.
- [X] T030 [P] [US3] Implement `ChatCompletionRequestDto`, `MessageDto`, `ChatCompletionResponseDto`, and its choice/message DTOs in `src/api/dto.rs`, with `stream` defaulting false and no success `usage` or `created` field.
- [X] T031 [US3] Implement the chat handler in `src/api/chat.rs` by mapping API DTOs to application commands, checking streaming only after minimum validation succeeds, invoking the mock service, and mapping the application result to the exact response DTO without importing domain types.
- [X] T032 [US3] Add the stateless mock service to `AppState` in `src/application.rs` and register `POST /v1/chat/completions` with `ChatAdmissionMiddleware` in `src/api/server.rs`.

**Checkpoint**: T027 and T028 pass; the primary end-to-end mock chat journey works locally with no provider setup.

---

## Phase 6: User Story 4 - Receive Predictable API Errors (Priority: P2)

**Goal**: Normalize every supported client and internal failure to the exact flat, safe `{code,message}` contract with correct routing and media semantics.

**Independent Test**: Exercise all seven error-contract rows without an external service and verify exact status, code, message, field set, no sensitive diagnostics, and body-suppressed HEAD responses.

### Tests for User Story 4

> Write T033 and T034 first and confirm they fail for the incomplete error mappings and routing behavior.

- [X] T033 [P] [US4] Add failing end-to-end error matrix tests in `tests/http_api.rs` for malformed/empty/non-object/missing-field chat input, routing/admission-before-media and structurally-invalid-before-stream precedence, streaming, media type, unsupported methods on each known path including body-suppressed HEAD and exact `Allow` headers, unknown paths, shutdown rejection, and a test-only injected internal failure, asserting exact status/code/message/fields and absence of prompts, credentials, source errors, or diagnostics.
- [X] T034 [P] [US4] Add failing unit tests in `src/api/error.rs` for `JsonRejection` status/media-type classification, fixed safe messages, and the guarantee that parser/source diagnostics never enter the response.

### Implementation for User Story 4

- [X] T035 [US4] Implement safe JSON rejection and media-type classification helpers in `src/api/error.rs`, mapping unreadable/structural bodies to 400 `invalid_request`, unsupported or missing media types to 415 `unsupported_media_type`, and discarding parser/source diagnostics.
- [X] T036 [US4] Replace convenience method routing and framework fallbacks in `src/api/server.rs` with exact GET/POST dispatch, the documented `Allow` headers, and a global 404 fallback while preserving HTTP body suppression for HEAD.
- [X] T037 [US4] Integrate normalized extractor and application errors in `src/api/chat.rs`, ensure the documented validation precedence, and expose no error details beyond the centralized `ApiErrorDto`.

**Checkpoint**: T033 and T034 pass; all seven error rows are stable, body-returning methods carry the exact JSON object, and HEAD carries status/headers without a body.

---

## Phase 7: Polish and Cross-Cutting Concerns

**Purpose**: Align repository documentation and governance contracts with the implemented service, then run the complete definition-of-done gates.

- [ ] T038 [P] Update `README.md` and `docs/api.md` with the long-running local gateway, all three endpoints, exact mock response, fixed errors, offline behavior, and the intentionally local unauthenticated scope.
- [ ] T039 [P] Update `docs/configuration.md` and `docs/testing.md` with `AI_GATEWAY_HOST`, `AI_GATEWAY_PORT`, defaults, validation failures, endpoint test strategy, lifecycle tests, and performance measurements.
- [ ] T040 [P] Record the HTTP Gateway Core implementation and operational handoff in `CHANGELOG.md` and `HANDOFF.md`, including startup, probing, chat, shutdown, and known phase limitations.
- [ ] T041 [P] Mark Phase 2 complete in `docs/plan.md` and align its milestone summary with the implemented three-endpoint service.
- [ ] T042 [P] Update `specs/001-project-foundation/contracts/environment.md` to record the two `AI_GATEWAY_` listener variables, defaults, validation, and `.env.example` non-loading rule.
- [ ] T043 [P] Update `specs/001-project-foundation/contracts/quality-gates.md` so the smoke run starts the long-lived service, verifies health/readiness, terminates it cleanly, and preserves the full Rust quality sequence.
- [ ] T044 [P] Update `specs/002-layered-architecture/contracts/layout.md` for the API application/config, application domain/config, config standard-library, and domain transport-free dependencies introduced by this phase.
- [ ] T045 [P] Reconcile `specs/002-layered-architecture/data-model.md` with `ChatResponse.usage: Option<Usage>`, the no-usage constructor, and the Phase 2 mock behavior.
- [ ] T046 Audit `Cargo.toml`, `src/api.rs`, `src/api/chat.rs`, `src/api/dto.rs`, `src/api/error.rs`, `src/api/health.rs`, `src/api/middleware.rs`, `src/api/server.rs`, `src/application.rs`, `src/application/chat.rs`, `src/application/lifecycle.rs`, `src/config.rs`, `src/config/server.rs`, `src/domain.rs`, `src/domain/chat.rs`, and `src/infrastructure.rs` to remove any API-to-domain, application-to-API, domain-to-transport, provider, credential, persistence, streaming, routing, or telemetry dependency that violates the approved scope.
- [ ] T047 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo build`, and `cargo build --release` for `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/main.rs`, and `tests/http_api.rs`/`tests/server_lifecycle.rs`, fixing every failure before completion.
- [ ] T048 Follow every scenario in `specs/003-http-gateway-core/quickstart.md` from a clean local build, including invalid configuration, alternate port, clean SIGTERM shutdown, offline operation, 100-request performance checks, and cleanup, and confirm the complete journey takes under 10 minutes without credentials or external services.

---

## Dependencies and Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies and may begin immediately.
- **Foundational (Phase 2)**: Depends on Setup and blocks every user story.
- **User Stories (Phases 3-6)**: Each depends on Foundational; sequential delivery follows P1 (`US1` → `US2` → `US3`) before P2 (`US4`).
- **Polish (Phase 7)**: Depends on all four user stories and their independent validation.

### User Story Dependencies

- **US1 (P1)**: No dependency on another user story after Foundational; delivers the MVP liveness slice.
- **US2 (P1)**: Uses the existing health route for its state-pair checks but owns all readiness and shutdown behavior; it has no provider dependency.
- **US3 (P1)**: Uses Foundational lifecycle/admission primitives and may be implemented after US2 so the production Ready and shutdown paths are already available; its independent chat test does not require external services.
- **US4 (P2)**: Can be implemented after Foundational, but sequential delivery after US3 validates the complete error matrix against all real routes and lifecycle states.

### Critical Paths

- Configuration and lifecycle primitives: T005-T008 and T013.
- HTTP runtime: T014-T022.
- Readiness and graceful shutdown: T023-T026.
- Mock chat path: T027-T032.
- Complete error contract: T033-T037.
- Definition of done: T038-T048.

### Parallel Opportunities

- Run T002, T003, and T004 in parallel after T001.
- Run foundational test-definition tasks T005, T007, T009, and T011 in parallel, followed by their implementation tasks.
- Run T033 and T034 together before User Story 4 implementation.
- Run independent documentation updates T038-T045 in parallel after implementation and tests are stable.
- Do not run tasks that edit the same file in parallel, especially `src/api/server.rs`, `src/api/dto.rs`, `src/application.rs`, and `tests/http_api.rs`.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Setup and Foundational phases.
2. Complete User Story 1 and stop at its checkpoint.
3. Confirm T018 passes, `cargo test --all-targets` remains green, and a real process serves `GET /health` until signaled.

### Incremental Delivery

1. Setup + Foundational → shared HTTP/lifecycle foundation.
2. US1 → independently validate and demo liveness.
3. US2 → independently validate readiness and graceful shutdown.
4. US3 → independently validate the primary mock chat journey.
5. US4 → independently validate predictable errors.
6. Polish → align documentation/contracts and pass all quality and quickstart gates.

### Parallel Team Strategy

1. Complete shared scaffolding, configuration, lifecycle, domain, DTO, error, and admission work together.
2. Assign separate ownership of `tests/http_api.rs`, `tests/server_lifecycle.rs`, application chat logic, API DTOs, and documentation only where file ownership does not overlap.
3. Integrate the shared `src/api/server.rs` router and shutdown coordinator serially after story components are ready.

---

## Requirements Traceability

| Requirement | Primary tasks |
|-------------|---------------|
| FR-001 | T019-T020, T025, T031-T032 |
| FR-002 | T018-T022 |
| FR-003 | T018, T020-T022 |
| FR-004 | T023, T025-T026 |
| FR-005 | T023, T025-T026 |
| FR-006 | T023, T025 |
| FR-007 | T027-T031 |
| FR-008 | T028, T030-T032 |
| FR-009 | T027-T029, T032 |
| FR-010 | T009-T010, T028, T030-T032 |
| FR-011 | T028, T033, T035, T037 |
| FR-012 | T033, T035-T037 |
| FR-013 | T014-T015, T033-T037 |
| FR-014 | T014-T015, T033 |
| FR-015 | T018, T027-T032 |
| FR-016 | T016-T017, T024, T026, T033 |
| FR-017 | T005-T006, T021-T026 |
| FR-018 | T047-T048 |
| FR-019 | T018, T023-T024, T028, T033-T034, T047 |
| FR-020 | T038-T041, T048 |
| SC-001 | T048 |
| SC-002 | T018, T023-T024, T028, T033-T034, T047 |
| SC-003 | T024 |
| SC-004 | T028 |
| SC-005 | T033-T035, T047 |
| SC-006 | T023-T024 |
| SC-007 | T024, T026 |
| SC-008 | T028, T048 |

---

## Notes

- `[P]` means different files and no unfinished dependency; it is not a general indication of low effort.
- Tests are written before the corresponding implementation and must fail for the intended reason, not because of an unrelated compile error.
- Commit only when explicitly requested; otherwise complete logical task groups without creating commits.
- Stop at each user-story checkpoint and verify its independent test before continuing.
- Keep all HTTP serialization in `src/api/dto.rs`, all minimum business validation in `src/application/chat.rs`, and the domain free of Axum, Tokio, and Serde.
