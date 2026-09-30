---

description: "Task list for feature 005 - Provider Abstraction"
---

# Tasks: Provider Abstraction

**Input**: Design documents from `/specs/005-provider-abstraction/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Test tasks ARE included. Constitution principle IX requires automated tests for every significant component, and FR-015, FR-016, FR-020 require regression protection that only tests can provide.

**Baseline**: Phase 3 ended at **203 passing tests** (141 library, 57 `http_api`, 5 `server_lifecycle`). This feature ADDS coverage. No existing test may be renamed, deleted, or weakened.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single project: `src/`, `tests/` at repository root. Layered per
`specs/002-layered-architecture/contracts/layout.md`.

## Critical Architecture Constraints

These are not stylistic. Violating them breaks a ratified contract.

1. `application` MUST NOT import `infrastructure` (layout rule 4). The provider
   **trait** therefore lives in `src/application/chat.rs`; only implementations
   live in `src/infrastructure/`.
2. `api` MUST NOT import `domain` types (layout rule 3). The API layer sees
   application types only.
3. `domain` stays free of `async`, `tokio`, and `serde` (layout rule 6).
   `ChatRequest`, `ChatResponse`, and `Usage` are reused **unchanged**.
4. The gateway serves exactly **three routes**. No new endpoint (FR-008 is
   satisfied through the existing readiness surface).
5. `main.rs` is the composition root (layout rules 1, 4, 9). `AppState` gains a
   provider field; `AppState::new(version)` MUST keep working with the
   deterministic provider so the existing 203 tests do not need editing.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependency and the infrastructure module tree

- [X] T001 Add `async-trait` to `[dependencies]` in `Cargo.toml` and commit the resulting `Cargo.lock` change. Required because RPITIT is not dyn-compatible, so `Arc<dyn LlmProvider>` cannot be built without it (research.md D-002)
- [X] T002 [P] Replace the placeholder module declaration in `src/infrastructure.rs` with a `pub mod providers;` declaration, keeping the existing doc comment about layer rules
- [X] T003 Create `src/infrastructure/providers/mod.rs` with a module doc comment stating that provider implementations live here, that the trait is defined in `application`, and that this layer may depend only on `domain`, `config`, and `std`
- [X] T004 [P] Create `tests/provider_abstraction.rs` with a module doc comment recording the Phase 3 baseline of 203 tests and stating that this file holds boundary, failure, concurrency, and extensibility tests, which use no network. **All tests in this file must be written into this one file**, because each `tests/*.rs` is a separate crate and a test double defined in one cannot be imported by another

**Checkpoint**: `cargo check --all-targets` still passes and the test count is still exactly 203

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The provider contract, its failure type, configuration, and the error rows every story depends on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T005 Add `ProviderFailure` as a fieldless `Copy` enum in `src/application/chat.rs` with exactly the five variants `Unreachable`, `Refused`, `DeadlineExceeded`, `UnusableResponse`, `InvalidResponse`, deriving `Debug, Clone, Copy, PartialEq, Eq`. It MUST NOT carry a message, status code, URL, model name, or any string, so provider detail is unrepresentable rather than merely filtered (FR-010, FR-012, data-model.md §3)
- [X] T006 Define `LlmProvider` in `src/application/chat.rs` using `#[async_trait]`, with `Send + Sync` supertraits, an `async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderFailure>`, and `fn id(&self) -> &str`. Neither signature may mention a provider-native type (FR-001, FR-002, FR-003, contracts/provider-contract.md §1)
- [X] T007 Add `ProviderConfig` to `src/config/server.rs` or a new `src/config/provider.rs` re-exported from `src/config.rs`, holding `provider_id: String` and `deadline_ms: u64`, read from `AI_GATEWAY_PROVIDER` and `AI_GATEWAY_PROVIDER_TIMEOUT_MS` with the deterministic provider as the default. Parse and range-validate at startup; an unparsable or non-positive value MUST return a `thiserror` error rather than falling back to a default (FR-005, FR-011, FR-028, research.md D-005)
- [X] T008 Add unit tests in `src/config/provider.rs` covering: default when unset, explicit valid value, unknown provider name, invalid deadline string, zero deadline, and non-UTF-8 handling consistent with the existing `ServerConfig` tests in `src/config/server.rs`
- [X] T009 Add `ApiError::ProviderUnavailable` (502, `provider_unavailable`, `The upstream provider is unavailable.`) and `ApiError::ProviderTimeout` (504, `provider_timeout`, `The upstream provider did not respond in time.`) to `src/api/error.rs`, extending `status_code()`, `code()`, and `message()` which each currently match all eight existing variants exhaustively. Messages MUST be fixed strings with no interpolation (FR-027, contracts/http-api.md §2)
- [X] T010 Add `ApiError::from_provider_failure` to `src/api/error.rs` mapping `Unreachable` and `Refused` to `ProviderUnavailable`, `DeadlineExceeded` to `ProviderTimeout`, and `UnusableResponse` and `InvalidResponse` to the existing `InternalError`. No category may map to a validation code (FR-012, FR-013)
- [X] T011 Add a test in `src/api/error.rs` asserting all ten codes are distinct, that every one of the five failure categories maps to the documented status and code, and that the eight Phase 3 rows are byte-identical to their current values
- [X] T012 Change the `chat_service` field on `AppState` in `src/application.rs` to hold `Arc<dyn LlmProvider>`, and add `AppState::new_with_provider(version, provider)`. **`AppState::new(version)` MUST keep compiling and MUST default to the deterministic provider**, so the existing 203 tests need no edits. Update the `use` of `MockChatCompletionService` accordingly
- [X] T013 Add the bounded provider call to `src/application/chat.rs`: a method that takes a `&ChatRequest` and the configured `deadline_ms`, wraps the provider future in a bounded wait, and returns `Result<ChatResponse, ProviderFailure>` with `DeadlineExceeded` when the bound elapses first. The bound MUST be applied here rather than in any adapter, so an adapter cannot omit it (FR-011, research.md D-004)
- [X] T014 Add a unit test in `src/application/chat.rs` asserting a provider that never resolves produces `DeadlineExceeded` within a bounded time, proving the call cannot hang (FR-011)
- [X] T015 Re-export `ProviderFailure` and `LlmProvider` from `src/application/chat.rs`'s public surface and confirm `src/lib.rs` needs no change because it already declares `pub mod infrastructure`

**Checkpoint**: `cargo test --all-targets` passes with the count **at least 203**, the trait compiles behind `Arc<dyn LlmProvider>`, and the bounded call is proven not to hang

---

## Phase 3: User Story 1 - Gateway Operators Route a Request to a Chosen Provider (Priority: P1) 🎯 MVP

**Goal**: An operator selects the provider from the environment at startup, an unusable selection is refused with a message naming the value, and the selection is discoverable through the existing readiness surface

**Independent Test**: Start the gateway with no provider variable and confirm it serves traffic and names the selected provider at readiness. Then start with an unknown provider name and confirm the process refuses to start and names it. No code change is needed to switch providers.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T016 [US1] Unit tests in `src/infrastructure/providers/mod.rs` for provider resolution: default resolves to the deterministic provider, a known id resolves to that provider, an unknown id fails with a message containing the requested value, and a disabled id fails with a message containing the requested value (FR-005, FR-007)
- [ ] T017 [US1] Config-validation tests in `tests/provider_abstraction.rs` asserting an unknown `AI_GATEWAY_PROVIDER`, an invalid `AI_GATEWAY_PROVIDER_TIMEOUT_MS`, and a zero deadline each produce a startup error naming the offending value, and that no network access is required (FR-007, FR-028)
- [ ] T018 [US1] Readiness-discoverability test in `tests/http_api.rs` asserting the readiness response reports the selected provider id (FR-008). Follow the existing `readiness_response_serializes_ready_status` pattern in `src/api/dto.rs` and add a `provider` field to `ReadinessResponseDto` in `src/api/dto.rs`
- [ ] T019 [US1] Default-no-configuration test in `tests/provider_abstraction.rs` asserting the gateway resolves and serves with zero environment variables set and performs no outbound connection (FR-006, constitution principle IX)

### Implementation for User Story 1

- [ ] T020 [US1] Create `ProviderRegistry` in `src/infrastructure/providers/mod.rs` mapping a provider id to its `Arc<dyn LlmProvider>`, with the deterministic provider registered and enabled, and resolution returning a `thiserror` error naming the requested id for unknown or disabled entries (FR-007, research.md D-005)
- [ ] T021 [US1] Add the `provider` field to `ReadinessResponseDto` in `src/api/dto.rs` and populate it in the `readiness` handler in `src/api/health.rs` from `state`. Update the two existing readiness serialization tests in `src/api/dto.rs` to include the field rather than deleting them (FR-008)
- [ ] T022 [US1] Wire startup resolution in `src/main.rs`: read `ProviderConfig`, resolve the provider through `ProviderRegistry`, construct `AppState` via `AppState::new_with_provider`, and return a context-annotated error that names the requested provider on failure. `main.rs` is the composition root and is the only place permitted to know both layers (FR-007, layout rules 1, 4, 9)
- [ ] T023 [US1] Verify the full quality gate passes and the test count is at least 203 with zero failures, confirming startup selection works end to end (SC-007)

**Checkpoint**: User Story 1 is fully functional and testable independently. The gateway starts with zero configuration, refuses an unusable selection by name, and reports its selection at readiness

---

## Phase 4: User Story 2 - Client Applications See One Stable Response Shape (Priority: P1)

**Goal**: The response body is identical regardless of which provider served it, and every provider failure reaches the client as one of the two documented codes with no provider detail

**Independent Test**: Serve the same request through two different providers and assert the response bodies are byte-identical. Then drive each of the five failure categories and assert the documented status, code, and message, with no provider detail present.

### Tests for User Story 2 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T024 [US2] Byte-identity test in `tests/provider_abstraction.rs` asserting that two providers returning equivalent completions produce byte-identical client-visible response bodies, and that the body has exactly the four keys `id`, `object`, `model`, `choices` with no `usage` and no `created` (FR-016, SC-002, research.md D-007)
- [ ] T025 [US2] Contract test in `tests/http_api.rs` asserting all five failure categories map to the documented status and code, and that the body **equals** the fixed string exactly rather than merely containing the code (FR-012, contracts/http-api.md §3)
- [ ] T026 [US2] No-leak test in `tests/http_api.rs` asserting that for every failure category the response body contains no provider status code, no provider message text, no provider URL, no provider model name, and no credential (FR-012, SC-003)
- [ ] T027 [US2] Precedence test in `tests/provider_abstraction.rs` asserting a request that fails Phase 3 validation and a request exceeding the size limit are both refused **without the provider being contacted**, so a malformed request can never cause a billable upstream call (contracts/http-api.md §6, invariants §1)
- [ ] T028 [US2] No-fabricated-usage test in `tests/provider_abstraction.rs` asserting a provider reporting no usage produces a response with no `usage` key and no invented token counts (FR-014)

### Implementation for User Story 2

- [ ] T029 [P] [US2] Create `src/infrastructure/providers/deterministic.rs` implementing `LlmProvider` for a `DeterministicProvider` that returns the same fixed completion the mock returns today, built via the existing `ChatResponse::without_usage`, and whose `id()` returns the registered identifier (FR-006, FR-020)
- [ ] T030 [US2] Create a test-only double in `tests/provider_abstraction.rs` implementing `LlmProvider`, configurable to return a success or to fail with any one of the five categories, and a second variant returning a fixed different completion. These MUST be reachable only from tests and MUST NOT be registered in shipped configuration (FR-006a, research.md D-006)
- [ ] T031 [US2] Replace the handler body in `src/api/chat.rs` to call the bounded provider call on `AppState` and to build the existing `ChatCompletionResponseDto` from the returned `ChatResponse`, mapping any `ProviderFailure` through `ApiError::from_provider_failure`. The response DTO itself MUST NOT change; only its construction path does (FR-016, contracts/http-api.md §5)
- [ ] T032 [US2] Make `MockChatCompletionService::complete` in `src/application/chat.rs` delegate to the provider contract, or remove it once nothing references it, ensuring the observable response is byte-identical to the Phase 3 output. Update any in-module unit tests in `src/application/chat.rs` to construct a provider rather than the removed concrete type, without weakening their assertions (FR-015, FR-020)
- [ ] T033 [US2] Run the complete Phase 3 error-row regression check from `quickstart.md` §5.1 and confirm all ten Phase 3 rows are byte-identical to their Phase 3 values, that `temperature: 2.0` and `max_tokens: 4096` remain accepted, and that `max_tokens: 4095` remains refused (FR-015, SC-006)
- [ ] T034 [US2] Assert in `tests/http_api.rs` that every error body has exactly two keys and contains no `details`, no wrapper, and no request field name, extending the existing `every_failure_response_has_exactly_two_fields_and_no_details` coverage to the two new rows (FR-027)

**Checkpoint**: User Stories 1 AND 2 both work independently. The response is provider-independent and every failure category is safely translated

---

## Phase 5: User Story 2 continued - Concurrency and Containment (Priority: P1)

**Goal**: Concurrent requests stay isolated across providers, and a failure never takes the gateway down

**Independent Test**: Issue concurrent requests across two providers and assert each receives its own provider's response. Then induce a failure and confirm health, readiness, and a subsequent request still work.

- [ ] T035 [US2] Concurrency-isolation test in `tests/provider_abstraction.rs` issuing 50 concurrent requests split across two providers, asserting every response came from the correct provider with zero cross-contamination and no request served by the wrong provider (FR-017, SC-005). Follow the existing `fifty_simultaneous_mixed_requests_stay_isolated` pattern in `tests/http_api.rs`
- [ ] T036 [US2] Determinism test in `tests/provider_abstraction.rs` asserting identical input to the deterministic provider yields a byte-identical response across repeated and concurrent calls, so the gateway's own suite remains a reliable regression net (FR-020)
- [ ] T037 [US2] Containment test in `tests/http_api.rs` asserting that after an induced provider failure, `/health` and `/ready` still return `200` and a subsequent valid chat request still succeeds, and that the process is still running (FR-018, SC-009)
- [ ] T038 [US2] Graceful-shutdown test in `tests/server_lifecycle.rs` asserting a request in flight when shutdown begins is allowed to finish or is abandoned deliberately, and that no new provider call is accepted afterwards, following the existing lifecycle test patterns in that file (FR-019)

**Checkpoint**: Concurrency and containment verified

---

## Phase 6: User Story 3 - Gateway Developers Add a Provider Without Touching the Gateway (Priority: P2)

**Goal**: A provider is added by writing one self-contained adapter and registering it, with no change to validation, the response, or any other provider

**Independent Test**: Add a test-only provider, confirm the suite passes with it, remove it, and confirm no other file required an edit.

### Tests for User Story 3 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T039 [US3] One-file-extension test in `tests/provider_abstraction.rs` implementing a minimal new provider in that file only, asserting the gateway serves through it, then asserting removing the implementation requires no edit elsewhere (FR-021, SC-001)
- [ ] T040 [US3] Boundary-isolation test in `tests/provider_abstraction.rs` asserting a provider-native request or response type never appears above the adapter boundary and that the domain types `ChatRequest` and `ChatResponse` gained no provider-specific fields (FR-003, FR-004, FR-022, SC-008)
- [ ] T041 [US3] Adapter-obligation test in `tests/provider_abstraction.rs` asserting a test provider configured to fail in each of the five categories produces the correct category, and that no provider detail reaches the caller (contracts/provider-contract.md §3, FR-010)

### Implementation for User Story 3

- [ ] T042 [US3] Register the deterministic provider and expose a single registration point in `src/infrastructure/providers/mod.rs` such that adding a provider is a self-contained edit to the infrastructure layer (FR-021)
- [ ] T043 [US3] Perform the one-file-extension exercise from `quickstart.md` §9 and record the result, confirming that adding a provider required no change to `src/application/chat.rs`, `src/api/chat.rs`, or any other provider (FR-021, SC-001)
- [ ] T044 [US3] Add a credential-leak test in `tests/provider_abstraction.rs` asserting a credential placed in the environment for a test provider appears in no log output, no response body, and no error message, satisfying the constitution's LLM Provider Integration standard even though no hosted adapter ships in this phase (FR-023, contracts/provider-contract.md §3)

**Checkpoint**: All three user stories are independently functional

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, contract synchronization, and full validation

- [ ] T045 [P] Update `docs/api.md` to reflect ten error codes: add `502 provider_unavailable` and `504 provider_timeout` to the §21 Validation Errors table, move both statuses out of the "Planned" table in §20, update the §20 phase note, and update the top status banner which currently states the contract has eight codes (FR-027, contracts/http-api.md §1)
- [ ] T046 [P] Update `README.md` with the provider abstraction, the new `infrastructure/providers/` module, and the `async-trait` dependency, following the existing "Current Module Structure" section format
- [ ] T047 [P] Update `CHANGELOG.md` with a Phase 4 entry under `### Added`, matching the phrasing and placement conventions established by Phases 0 through 3
- [ ] T048 [P] Update `HANDOFF.md`: mark Phase 4 complete, record the ten-code error contract, record that `AI_GATEWAY_PROVIDER` and `AI_GATEWAY_PROVIDER_TIMEOUT_MS` are the new environment variables, note that live provider connectivity is Phase 5, and update Next Steps to target `docs/plan.md` section 10
- [ ] T049 [P] Add a supersession note to `specs/005-provider-abstraction/contracts/http-api.md` reference in `specs/004-domain-validation/contracts/http-api.md`, following the pattern already used there to prevent the two contracts from contradicting each other
- [ ] T050 [P] Update `.env.example` with the two new environment variables, documenting them as optional and showing their defaults
- [ ] T051 Verify the layering contract still holds: confirm `src/application/` contains no `use crate::infrastructure`, `src/api/` contains no `use crate::domain`, and `src/domain/` contains no `tokio`, `async_trait`, or `serde` import. This is the mechanical check of layout rules 2, 3, 4, and 6 (constitution principle V). **Additionally confirm the rule 9 direction holds: `src/infrastructure/` may import `application::chat::LlmProvider` for the port but MUST NOT call application orchestration, so no `ChatValidator` or `AppState` import appears there**
- [ ] T052 Run the complete `quickstart.md` validation, including all six quality gates, and confirm every one of the ten completion criteria in its section holds
- [ ] T053 Confirm the final test count is at least 203 with zero failures and that no Phase 3 test was renamed, deleted, or weakened, recording the before and after counts
- [ ] T054 Update `specs/005-provider-abstraction/checklists/requirements.md` notes with the implemented error-code count and the actual test total, so the record reflects what shipped rather than what was planned

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup - BLOCKS all user stories
- **User Stories (Phases 3-6)**: All depend on Foundational
  - US1 and US2 are both P1 and can proceed in parallel once Phase 2 completes
  - US3 is P2 and depends on US1 and US2 being functional
- **Polish (Phase 7)**: Depends on all desired stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Starts after Foundational. No dependency on other stories
- **User Story 2 (P1)**: Starts after Foundational. Depends on the trait and error rows from Foundational, and on US1 only for the wiring that US1 puts in `main.rs`; the response-shape and failure-translation work itself is independent
- **User Story 2 continued (Phase 5)**: Depends on US2
- **User Story 3 (P2)**: Starts after US1 and US2. Proves their combination, so it is not independently shippable

### Within Each User Story

- Tests are written and MUST FAIL before implementation
- Contract and error definitions before handlers
- Adapters before the registry registers them
- Core implementation before integration

### Parallel Opportunities

- All Setup tasks marked [P] run in parallel
- All Foundational tasks marked [P] run in parallel
- Once Foundational completes, US1 and US2 can start in parallel
- All tests for a story marked [P] run in parallel
- All Polish documentation tasks marked [P] run in parallel

---

## Parallel Example: Phase 2 Foundational

```bash
# Genuinely concurrent: different files, no shared dependency
Task: "T002 [P] Declare the providers module in src/infrastructure.rs"
Task: "T004 [P] Create tests/provider_abstraction.rs"

# Then sequentially within src/api/error.rs:
Task: "T009 Add ApiError::ProviderUnavailable and ProviderTimeout in src/api/error.rs"
Task: "T010 Add ApiError::from_provider_failure in src/api/error.rs"
```

Note: T009 and T010 both touch `src/api/error.rs`, so neither is marked [P].
The two error.rs tasks are consecutive, so one agent can take both.

## Parallel Example: User Story 2

```bash
# Concurrent: the adapter is a new file, the contract test extends an existing one
Task: "T025 Contract test for all five failure categories in tests/http_api.rs"
Task: "T029 [P] Create src/infrastructure/providers/deterministic.rs"

# Sequential within tests/provider_abstraction.rs (single crate, one file):
Task: "T024 Byte-identity test"
Task: "T027 Precedence test, no provider call before validation"
Task: "T028 No-fabricated-usage test"
Task: "T030 Add the test-only provider double"
```

Note: within any one phase, tasks sharing a file are deliberately **not**
marked [P] and must run sequentially. Each `tests/*.rs` is a separate crate, so
a test double built in one file cannot be imported by another; all test-double
work therefore lives in the single `tests/provider_abstraction.rs` created in
T004.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: confirm the gateway starts with zero configuration,
   refuses an unusable provider selection by name, and reports its selection
5. Deploy or demo

### Incremental Delivery

1. Setup + Foundational -> foundation ready, trait usable
2. US1 -> operator can select a provider -> validate -> demo
3. US2 -> response is provider-independent and failures are safe -> validate
4. US3 -> extensibility proven -> validate
5. Polish -> documentation and contracts synchronized

### Parallel Team Strategy

1. Team completes Setup + Foundational together
2. Then: Developer A on US1, Developer B on US2, since both are P1 and
   independent apart from the shared `main.rs` wiring in T022
3. US3 and Polish after both land

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently
- Avoid: vague tasks, same-file conflicts marked [P], cross-story dependencies
  that break independence
- **Do not mark a task complete on a test that merely passes.** SC-002 and
  SC-003 require byte-equality assertions, not containment.
- `docs/api.md` currently states eight error codes. Until T045 lands, that
  document and the new contract disagree, which is expected mid-implementation
  and is called out in `quickstart.md` §10.
