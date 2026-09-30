# Tasks: Domain Model Validation

**Input**: Design documents from `/specs/004-domain-validation/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/validation-rules.md, contracts/http-api.md, quickstart.md

**Tests**: Included. FR-026 and SC-002 explicitly require automated coverage of every documented rule, every rule's failure response, the precedence order, and the size boundary, so test tasks are mandatory in this feature rather than optional.

**Organization**: Tasks are grouped by user story. The five stories come from spec.md with priorities US1 P1, US2 P1, US3 P1, US4 P2, US5 P2. The MVP cut line is the end of Phase 5 (all P1 stories).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel — a different file from every other `[P]` task in the same phase, and no dependency on an incomplete task.
- **[Story]**: Which user story this task belongs to. Required in user-story phases, absent in Setup, Foundational, and Polish.
- **File path**: Every description names the exact file it changes or verifies. A task without one is invalid.
- **Checkbox**: Every task begins `- [ ]` in execution order.

## Path Conventions

- **Single project** (this feature): `src/` and `tests/` at the repository root. Unit tests live in the `#[cfg(test)]` module of the file they cover; integration tests live in `tests/http_api.rs`.
- **Web app** and **Mobile**: not applicable. `plan.md` records a single-crate layout, so the single-project convention is used throughout.
- `src/` file stems are module names: `src/api/chat.rs` is the `api::chat` module.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Capture a trustworthy baseline and add the one dependency this phase needs, before any behavior changes.

- [X] T001 Record the pre-change baseline: run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`, and confirm all three pass with the Phase 2 suite green. Record the passing test count for `tests/http_api.rs` and `tests/server_lifecycle.rs` plus every `src/` unit-test module. Every later "no regression" claim in this file is measured against this number (FR-025, SC-010).
- [X] T002 [P] Add `http-body-util = "0.1"` to `[dependencies]` in `Cargo.toml` so `bound_chat_body` can match on `http_body_util::LengthLimitError` instead of the opaque `axum::Error`. Confirm `cargo check` still succeeds.
- [X] T003 [P] Confirm `rust-toolchain.toml` pins 1.98.1 with edition 2024 and that `cargo build` succeeds. This phase introduces no new toolchain requirement and no new crate beyond T002.

**Checkpoint**: Baseline captured, dependency tree builds, existing suite green.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Build the single ordered validation spine that every user story depends on. After this phase, the gateway has exactly one place that decides whether a chat request is acceptable, and no observable behavior has changed.

- [X] T004 Add `ValidationFailure` to `src/application/chat.rs` as a fieldless enum with exactly the seven variants named in `data-model.md`: `InvalidModel`, `InvalidMessages`, `InvalidRole`, `InvalidContent`, `TemperatureOutOfRange`, `MaxTokensOutOfRange`, `StreamingRequested`. Carry no payload, so no client-supplied value can travel inside a failure (FR-018). Type-level, duplicate, and structural failures never reach this enum; they are decided during DTO deserialization.
- [X] T005 Replace `InvalidChatRequest` in `src/application/chat.rs` with `ValidationError { failure: ValidationFailure }`. Its `Display` output must remain exactly `The chat request is invalid.` for every non-stream failure and `Streaming is not supported.` for `StreamingRequested`, matching the two client-facing messages in `contracts/validation-rules.md` §4.
- [X] T006 Extend `CompleteChatCommand` in `src/application/chat.rs` with `temperature: Option<f64>`, `max_tokens: Option<u32>`, and `stream: bool`, so the command is a faithful carrier of the whole request vocabulary.
- [X] T007 Refactor `MockChatCompletionService::validate` in `src/application/chat.rs` into the ordered stage pipeline from `contracts/validation-rules.md` §3 — structural, then required-field, then control-range, then stream — returning `Result<ValidatedChat, ValidationError>`. Move the existing `model`/`messages`/role/`content` checks into their documented stages, preserving today's accept/reject behavior exactly and keeping the loop allocation-free of surprises (FR-014, FR-015, FR-016).
- [X] T008 [P] Add `ApiError::from_validation_error` to `src/api/error.rs`, mapping every non-stream failure to `InvalidRequest` and `StreamingRequested` to `UnsupportedFeature`. Leave all seven existing variants, their statuses, codes, and messages byte-identical (FR-017, FR-023).
- [X] T009 Update the handler in `src/api/chat.rs` to construct the full `CompleteChatCommand` and map failures through `ApiError::from_validation_error`. Keep the existing handler-level `if stream` check in place for now; T039 moves it into the pipeline. The only request type the handler constructs is the command, and the only request type `complete()` accepts is `ValidatedChat` (FR-014).
- [X] T010 Extend the unit tests in `src/application/chat.rs` to assert the pipeline preserves every existing outcome and that each `ValidationFailure` category renders its documented fixed message.
- [X] T011 Run `cargo test --all-targets` and confirm against the baseline from T001 that the observable behavior of `src/application/chat.rs`, `src/api/error.rs`, `src/api/chat.rs`, and `tests/http_api.rs` is unchanged: no existing test needed editing, and a client sending today's requests sees today's responses.

**Checkpoint**: One ordered validation spine exists, is the sole decision point, and changed no client-visible behavior.

---

## Phase 3: User Story 1 - Submit a Request with Generation Controls (Priority: P1) 🎯 MVP

**Goal**: Accept `temperature` and `max_tokens` when they are well-formed, and carry the exact submitted values into the validated request instead of discarding them or inventing a default.

**Independent Test**: Send a request with in-range controls and assert 200 plus the exact submitted values on the validated request; send the same request with the controls omitted and assert both are recorded as unspecified and the response is byte-identical.

### Tests for User Story 1 ⚠️

> Write these first and confirm they fail before implementing.

- [X] T012 [P] [US1] Add failing unit tests in `src/domain/chat.rs` for `ChatRequest::new` recording both controls as `None` and `ChatRequest::with_controls` recording exactly the supplied values without trimming or reordering.
- [X] T013 [P] [US1] Add failing unit tests in `src/application/chat.rs` asserting a command carrying `Some(0.7)` and `Some(500)` produces a validated request whose `temperature()` and `max_tokens()` return exactly those values, and a command carrying `None`/`None` returns `None` for both (FR-006, FR-007).
- [X] T014 [P] [US1] Add failing integration tests in `tests/http_api.rs` asserting the canonical request with in-range controls, with boundary controls `0.0`/`1` and `2.0`/`4096`, and with controls omitted all return 200 with byte-identical bodies and no echoed control (spec.md SC-003).

### Implementation for User Story 1

- [X] T015 [US1] Add `temperature: Option<f64>` and `max_tokens: Option<u32>` as flat fields on `ChatRequest` in `src/domain/chat.rs`. Keep `new(model, messages)` setting both to `None` so every existing construction site and existing test keeps compiling and comparing equal; add `with_controls(model, messages, temperature, max_tokens)` (spec.md FR-006; `docs/api.md` §34).
- [X] T016 [US1] Add `with_controls` unit tests to `src/domain/chat.rs` covering the `None` defaults of `new`, exact value retention, and `PartialEq` behavior now that the struct has four fields.
- [X] T017 [US1] Map the command's controls into the validated `ChatRequest` via `with_controls` in `src/application/chat.rs`, and add `temperature()`, `max_tokens()`, and `stream_requested()` accessors on `ValidatedChat` so tests can assert the submitted values without reaching into private state (FR-007).
- [X] T018 [P] [US1] Add `#[serde(default)] temperature: Option<f64>` and `#[serde(default)] max_tokens: Option<u32>` to `ChatCompletionRequestDto` in `src/api/dto.rs`, keeping the derived `Deserialize` for now. T023 replaces it.
- [X] T019 [US1] Add DTO unit tests in `src/api/dto.rs` asserting the controls deserialize when present, default to `None` when absent, and leave unknown fields such as `top_p` and `future_option` ignored (FR-020).
- [X] T020 [US1] Wire the DTO controls into `CompleteChatCommand` in `src/api/chat.rs`, and assert the mock completion is unchanged by the controls, since no model is connected and a response must never imply a control influenced it.

**Checkpoint**: US1 is fully functional and independently testable. This is the MVP cut line.

---

## Phase 4: User Story 2 - Reject Out-of-Range Generation Controls (Priority: P1)

**Goal**: Refuse a control that is out of range or not a well-formed value of the expected kind, with the documented 400 response and no disclosure of the submitted value or any parser detail.

**Independent Test**: Submit temperature below 0.0 and above 2.0, max_tokens of 0 and of 4097, and non-numeric values for both controls; assert every response is exactly 400 / `invalid_request` / `The chat request is invalid.`

### Tests for User Story 2 ⚠️

> Write these first and confirm they fail before implementing.

- [X] T021 [P] [US2] Add failing unit tests in `src/application/chat.rs` for the control-range stage: `0.0` and `2.0` accepted, `-0.1` and `2.0001` rejected; `1` and `4096` accepted, `0` and `4097` rejected; and each rejection yields `TemperatureRange` or `MaxTokensRange` and the fixed message (FR-004, FR-005, spec.md SC-004).
- [X] T022 [P] [US2] Add failing DTO tests in `src/api/dto.rs` asserting rejection of `"0.5"`, `true`, `null`, `{}`, and `[]` for `temperature`; `true`, `null`, `[500]`, and `100.5` for `max_tokens`; a body supplying `temperature` or `max_tokens` twice; and continued tolerance of unknown extra fields (FR-008, FR-009, FR-020).
- [X] T023 [P] [US2] Add failing integration tests in `tests/http_api.rs` for every control failure in `contracts/validation-rules.md` §5, each asserting exactly 400 / `invalid_request` / `The chat request is invalid.`, a request violating both a required field and a control range returning exactly one such response, and no response body containing the rejected value, a prompt marker, a field name, or parser text (FR-015, FR-018, spec.md SC-005).

### Implementation for User Story 2

- [X] T024 [P] [US2] Add `MIN_TEMPERATURE`, `MAX_TEMPERATURE`, `MIN_MAX_TOKENS`, and `MAX_MAX_TOKENS` constants to `src/domain/chat.rs` with the inclusive bounds `0.0`, `2.0`, `1`, and `4096`, and assert the bound values in that module's unit tests.
- [X] T025 [US2] Add the control-range stage to the ordered pipeline in `src/application/chat.rs`, placed after required-field validity and before the stream stage, and mapping each breach to its `ValidationFailure` category.
- [X] T026 [US2] Replace the derived `Deserialize` for `ChatCompletionRequestDto` in `src/api/dto.rs` with a manual `Deserializer` over `MapAccess`: ignore unknown keys, require a JSON number for `temperature` and a JSON integer for `max_tokens` via `serde_json::Value` so `null` and numeric strings are refused rather than treated as absent, and track seen control keys within the single call so a duplicate is refused with no shared state (FR-008, FR-009, FR-022). The custom deserializer error flows through the existing `JsonDataError` to `ApiError::InvalidRequest`, so a bad control can never surface as 500.

**Checkpoint**: US1 and US2 both work independently.

---

## Phase 5: User Story 3 - Submit a Reasonable-Sized Request (Priority: P1)

**Goal**: Accept a body of at most 1 MB whole, refuse anything larger with a distinct 413 response before any further validation, and leave the gateway healthy.

**Independent Test**: Submit a body of exactly 1 048 576 bytes and assert 200; submit 1 048 577 bytes and assert 413 with the documented code and message; then assert a normal request still succeeds.

### Tests for User Story 3 ⚠️

> Write these first and confirm they fail before implementing.

- [X] T027 [P] [US3] Add failing unit tests in `src/api/middleware.rs` for the size-bound stage: a declared `Content-Length` above the limit is refused without the body being read, a declared length exactly at the limit is admitted, and a body with no declared length that exceeds the limit while arriving is refused. Assert `MAX_REQUEST_BODY_BYTES` equals `1_048_576` (FR-010, FR-011, FR-012).
- [X] T028 [P] [US3] Add failing integration tests in `tests/http_api.rs` for a body of exactly 1 048 576 bytes returning 200 and 1 048 577 returning 413; the 413 body being exactly `payload_too_large` / `The request payload is too large.` with no count, length, or internal detail; health and readiness still correct and a normal request still served afterwards; an oversized body with `Content-Type: text/plain` still returning 413 and not 415; and an oversized body with no declared length refused while the gateway stays healthy (FR-010, FR-011, FR-012, FR-016, FR-018, FR-019, spec.md SC-004, SC-005, SC-009).

### Implementation for User Story 3

- [X] T029 [P] [US3] Add `ApiError::PayloadTooLarge` to `src/api/error.rs` with status 413, code `payload_too_large`, and message `The request payload is too large.`, extending all three match arms. Add a test using the module's existing `assert_error_contract` helper so the new row is held to the same two-field, no-`details` standard as the other seven (FR-017, FR-019, FR-023).
- [X] T030 [US3] Add `MAX_REQUEST_BODY_BYTES: usize = 1_048_576` and the `bound_chat_body` stage in `src/api/middleware.rs`: return `ApiError::PayloadTooLarge` immediately when a declared `Content-Length` exceeds the limit, otherwise read with `axum::body::to_bytes(body, limit)` and map `http_body_util::LengthLimitError` to `PayloadTooLarge`. Pass the bounded bytes to the next stage as the request body.
- [X] T031 [US3] Compose `bound_chat_body` inside the chat `admit_chat` layer in `src/api/server.rs` so it runs after admission and before the `Json` extractor. Axum's `Json` checks media type before buffering, so the limit must not live in `DefaultBodyLimit`, or an oversized body with a wrong media type would return 415 and break the documented precedence (FR-016). Confirm the admission guard is released on every path, including both refusal paths.
- [X] T032 [US3] Update `src/api/middleware.rs` unit tests to assert an over-limit refusal still releases the admission slot, verified through `lifecycle.wait_for_zero()`, so a refused oversized request cannot stall graceful shutdown (FR-012, FR-024).

**Checkpoint**: All P1 stories complete. This is the end of the MVP cut line.

---

## Phase 6: User Story 4 - Trust One Consistent Validation Contract (Priority: P2)

**Goal**: Every documented rule has an automated check, combined violations resolve by the documented precedence, and concurrency and determinism hold. The behavior already exists from Phases 3-5; this story is the proof that it is complete and stable.

**Independent Test**: Run the table-driven rule suite, the precedence suite, and the concurrency suite; all pass with no external service.

All tasks in this phase edit `tests/http_api.rs`, so they must be applied sequentially rather than in parallel.

- [X] T033 [US4] Add a table-driven test in `tests/http_api.rs` with one violating request per rule ID from `contracts/validation-rules.md` §2 — all 20 IDs — each asserting the exact status, code, and message from §4 (spec.md SC-002).
- [X] T034 [US4] Add a coverage assertion in `tests/http_api.rs` that the set of rule IDs exercised by T033 equals the full 20-ID catalog, so a rule added to the contract without a check fails the build (FR-026, SC-002).
- [X] T035 [US4] Add precedence tests in `tests/http_api.rs` for all nine stages of `contracts/validation-rules.md` §3, including a request violating model, message, control, and size rules at once, each request repeated 20 times asserting one stable documented response (FR-015, FR-016, spec.md SC-006).
- [X] T036 [US4] Add concurrency and determinism tests in `tests/http_api.rs`: 50 simultaneous mixed valid and invalid requests returning 50 isolated correct results, and 100 repetitions of the canonical request with and without controls returning identical status and body (FR-022, spec.md SC-003, SC-008).
- [X] T037 [US4] Add lifecycle tests in `tests/http_api.rs` asserting `/health` and `/ready` are correct before, during, and after valid, invalid, and oversized requests, and that an oversized request does not degrade later handling (spec.md SC-009, FR-024).

**Checkpoint**: US1-US3 remain green and the contract is fully covered.

---

## Phase 7: User Story 5 - Reject Explicit Streaming Requests (Priority: P2)

**Goal**: A request that explicitly asks for a stream receives the documented unsupported-feature response, decided inside the ordered pipeline as its last stage, and a request that omits the control is unaffected.

**Independent Test**: Submit a structurally valid request with `stream: true` and assert 400 / `unsupported_feature` / `Streaming is not supported.`; omit the control and assert 200.

### Tests for User Story 5 ⚠️

> Write these first and confirm they fail before implementing.

- [X] T038 [P] [US5] Add failing unit tests in `src/application/chat.rs` asserting a command with `stream: true` fails with `ValidationFailure::StreamingRequested` and renders `Streaming is not supported.`, and that a command with `stream: false` validates with `stream_requested()` returning `false` (FR-013).
- [X] T039 [P] [US5] Add failing integration tests in `tests/http_api.rs` for a structurally valid request with `stream: true` returning 400 / `unsupported_feature`; a request invalid on its own merits that also sets `stream: true` returning 400 / `invalid_request` instead; a request omitting `stream` returning 200; and the streaming refusal body containing no completion and no internal detail (spec.md US5 scenarios 1-4).

### Implementation for User Story 5

- [X] T040 [US5] Add the stream stage as the final stage of the ordered pipeline in `src/application/chat.rs`, after control-range validity, returning `ValidationFailure::StreamingRequested` and recording the flag on `ValidatedChat` so a validated request states whether a stream was requested (FR-013, FR-016).
- [X] T041 [US5] Map `ValidationFailure::StreamingRequested` to `ApiError::UnsupportedFeature` in `ApiError::from_validation_error` in `src/api/error.rs`, keeping its existing 400 status, `unsupported_feature` code, and message unchanged (FR-023).
- [X] T042 [US5] Remove the handler-level `if stream` check from `src/api/chat.rs` so the ordered pipeline is the only place streaming is decided, then confirm no other code path returns a completion for a stream request (FR-014, FR-015, FR-016).

**Checkpoint**: All five stories are independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T043 [P] Update `docs/api.md` §10, §11.3-§11.5, §20, §21, §34, and §40 to state the implemented rules, the inclusive ranges, the 1 MB whole-body limit with its inclusive boundary, the nine-stage precedence order, and the new `payload_too_large` row, matching `contracts/validation-rules.md` and `contracts/http-api.md` (FR-027, spec.md SC-001).
- [X] T044 [P] Update the chat-completions sections of `specs/003-http-gateway-core/contracts/http-api.md` to record that this feature's contract supersedes them for the chat endpoint, so the two documents do not contradict each other.
- [X] T045 [P] Update `docs/testing.md` with the new test groups — rule table, precedence, size boundary, concurrency, lifecycle — and the commands that run them.
- [X] T046 [P] Update `README.md`, `CHANGELOG.md`, `HANDOFF.md`, and the phase 3 entry in `docs/plan.md` to record the completed phase and the new error row.
- [X] T047 [P] Add a test in `tests/http_api.rs` asserting validation reaches no provider, credential, database, cache, or network resource, and that the gateway reports its own ability to serve honestly with no external configuration present (FR-021).
- [X] T048 Add a test in `tests/http_api.rs` asserting the implemented error contract has exactly the 14 rows of the Client-Facing Validation Contract table in `spec.md`, each covered and byte-exact, so the contract cannot drift silently (FR-017, FR-023, FR-027).
- [X] T049 Run the full quality gate over `Cargo.toml`, `src/`, and both integration suites `tests/http_api.rs` and `tests/server_lifecycle.rs`: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo build`, and `cargo build --release`. Confirm the suite count is at least the T001 baseline with no test removed (FR-025, SC-010).
- [X] T050 Execute all 15 scenarios in `specs/004-domain-validation/quickstart.md` against a running debug build, and record each expected-versus-actual result.
- [X] T051 Re-verify every row of the Client-Facing Validation Contract table in `spec.md` against a live response, then mark `specs/004-domain-validation/checklists/requirements.md` as verified.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. Can start immediately.
- **Foundational (Phase 2)**: Depends on Phase 1. **BLOCKS all user stories.**
- **User Stories (Phases 3-7)**: All depend on Phase 2. Sequentially in priority order P1 → P2, or in parallel across stories once the single-file ownership per story is respected.
- **Polish (Phase 8)**: Depends on all desired stories being complete.

### User Story Dependencies

- **US1 (P1)**: Starts after Phase 2. No dependency on another story. Adds the domain fields and the DTO fields the later stories tighten.
- **US2 (P1)**: Starts after Phase 2. Builds directly on US1's control fields; T026 replaces the deserializer US1 introduced, so US1 should land first even though US2 does not strictly require it.
- **US3 (P1)**: Starts after Phase 2. Independent of US1 and US2 in code; only shares `ApiError` and the chat layer composition.
- **US4 (P2)**: Starts after US1, US2, and US3, because its coverage table enumerates the rules those three stories create. Contains no implementation change.
- **US5 (P2)**: Starts after Phase 2. Independent of US1-US3 in code; only shares the pipeline and `ApiError` mapping.

### Within Each User Story

- Tests are written and confirmed to fail before the implementation task in the same story.
- Domain model before application pipeline before DTO before handler.
- Constants before the stage that uses them.
- Existing test count must not shrink at any point.

### Parallel Opportunities

- Phase 1: T002 and T003 in parallel.
- Phase 2: T008 in parallel with T004-T007; it touches a different file and neither reads nor writes the other's symbol.
- Phase 3: T012, T013, T014, and T018 in parallel — four different files.
- Phase 4: T021, T022, T023, and T024 in parallel — four different files.
- Phase 5: T027, T028, and T029 in parallel — three different files.
- Phase 6: no parallelism. Every task edits `tests/http_api.rs`.
- Phase 7: T038 and T039 in parallel — two different files.
- Phase 8: T043-T047 in parallel across distinct documentation and test files.

**File-ownership constraint**: within one phase, no two `[P]` tasks may edit the same file. This is why a task that follows another task in the same file is deliberately not marked `[P]`, even when it is a small edit.

### Cross-Story File Contention

Stories are logically independent, but three files are touched by more than one story. Serialize any pair that shares a file:

| File | Touched by |
|------|-----------|
| `src/application/chat.rs` | Foundational (T004-T007, T010), US1 (T013, T017), US2 (T021, T025), US5 (T038, T040) |
| `src/api/chat.rs` | Foundational (T009), US1 (T020), US5 (T042) |
| `src/api/error.rs` | Foundational (T008), US3 (T029), US5 (T041) |
| `src/api/dto.rs` | US1 (T018, T019), US2 (T022, T026) |
| `src/domain/chat.rs` | US1 (T012, T015, T016), US2 (T024) |
| `tests/http_api.rs` | US1 (T014), US2 (T023), US3 (T028), US4 (T033-T037), US5 (T039), Polish (T047, T048) |

`src/api/middleware.rs` and `src/api/server.rs` belong to US3 alone, which makes US3 the only story that can start without contending for a file an earlier story also needs.


---

## Parallel Examples by User Story

### User Story 1

```bash
# Launch together — four different files, no shared symbols:
Task: "T012 Add failing unit tests for ChatRequest::new defaults and with_controls in src/domain/chat.rs"
Task: "T013 Add failing unit tests for command-to-validated control carry-through in src/application/chat.rs"
Task: "T014 Add failing integration tests for in-range, boundary, and absent controls in tests/http_api.rs"
Task: "T018 Add serde(default) control fields to ChatCompletionRequestDto in src/api/dto.rs"

# Then, sequentially — each touches a file a parallel task above already edited:
Task: "T015 Add the control fields and with_controls to ChatRequest in src/domain/chat.rs"
Task: "T016 Add with_controls unit tests in src/domain/chat.rs"
Task: "T017 Carry controls into the validated request in src/application/chat.rs"
Task: "T019 Add DTO control deserialization tests in src/api/dto.rs"
Task: "T020 Wire DTO controls into CompleteChatCommand in src/api/chat.rs"
```

### User Story 2

```bash
# Launch together — four different files, no shared symbols:
Task: "T021 Add failing unit tests for the control-range stage in src/application/chat.rs"
Task: "T022 Add failing DTO tests for non-number, non-integer, and duplicate controls in src/api/dto.rs"
Task: "T023 Add failing integration tests for every control failure in tests/http_api.rs"
Task: "T024 Add control range bound constants in src/domain/chat.rs"

# Then, sequentially:
Task: "T025 Add the control-range stage to the ordered pipeline in src/application/chat.rs"
Task: "T026 Replace the derived Deserialize with a manual MapAccess impl in src/api/dto.rs"
```

### User Story 3

```bash
# Launch together — three different files, no shared symbols:
Task: "T027 Add failing unit tests for the size-bound stage in src/api/middleware.rs"
Task: "T028 Add failing integration tests for the 1 MB boundary and precedence in tests/http_api.rs"
Task: "T029 Add ApiError::PayloadTooLarge and its contract test in src/api/error.rs"

# Then, sequentially:
Task: "T030 Add MAX_REQUEST_BODY_BYTES and bound_chat_body in src/api/middleware.rs"
Task: "T031 Compose bound_chat_body inside the admit_chat layer in src/api/server.rs"
Task: "T032 Add admission-release tests for the refusal paths in src/api/middleware.rs"
```

### User Story 4

No parallel example, by design. T033 through T037 all edit `tests/http_api.rs`, so this story is a single sequential file. The story's value is coverage, not speed.

### User Story 5

```bash
# Launch together — two different files, no shared symbols:
Task: "T038 Add failing unit tests for the stream stage in src/application/chat.rs"
Task: "T039 Add failing integration tests for streaming precedence in tests/http_api.rs"

# Then, sequentially:
Task: "T040 Add the stream stage to the ordered pipeline in src/application/chat.rs"
Task: "T041 Map the stream failure to UnsupportedFeature in src/api/error.rs"
Task: "T042 Remove the handler-level stream check in src/api/chat.rs"
```

---

## Implementation Strategy

### MVP First (User Stories 1-3 only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational — **critical, blocks everything**.
3. Complete Phase 3: US1 — generation controls accepted and carried.
4. Complete Phase 4: US2 — bad controls refused.
5. Complete Phase 5: US3 — request size bounded.
6. **STOP and VALIDATE**: run the full quality gate and quickstart scenarios 1-9.

The P1 cut is a complete, shippable contract: a developer can send controls, bad controls are refused, and the body is bounded. US4 and US5 harden and formalize behavior that already works.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 → validate independently → MVP.
3. US2 → validate independently.
4. US3 → validate independently → full P1 contract complete.
5. US4 → coverage and precedence proven.
6. US5 → streaming formalized in the pipeline.
7. Polish → documentation, contract, and full verification.

### Parallel Team Strategy

Contention is real: US1, US2, and US5 all edit `src/application/chat.rs` and `src/api/chat.rs`, so they cannot be worked simultaneously. A workable split:

1. Team completes Setup + Foundational together. This is the shared contract.
2. Then:
   - Developer A: US1, then US2 — one continuous thread through the control vocabulary from domain to wire.
   - Developer B: US3 — owns `src/api/middleware.rs` and `src/api/server.rs`, which nothing else needs. Coordinate only the `ApiError` row with Developer C.
   - Developer C: waits for US1 to land, then takes US5. If US3's `ApiError::PayloadTooLarge` is not yet in `src/api/error.rs`, C can land T040 and T042 and merge the error row in review.
3. Whoever is free takes US4, the single-file coverage phase, after US1-US3 are all merged.
4. Integrate stories independently; the Foundational phase is what lets them compose at all.

For a single developer, the sequential order in "Incremental Delivery" is the only sensible one.


---

## Requirement Coverage

| Requirement | Tasks |
|-------------|-------|
| FR-001 one documented rule set | T007, T033, T034 |
| FR-002 model non-empty, preserved exactly | T007, T033 |
| FR-003 messages, roles, content, order | T007, T033 |
| FR-004 temperature 0.0-2.0 inclusive | T021, T025, T033 |
| FR-005 max_tokens 1-4096 inclusive | T021, T025, T033 |
| FR-006 absent controls recorded as unspecified | T012, T013, T015, T019 |
| FR-007 exact submitted value carried | T012, T013, T017 |
| FR-008 non-number controls refused, never 500 | T022, T023, T026 |
| FR-009 duplicate control refused | T022, T026 |
| FR-010 1 MB limit, distinct oversize response | T027, T028, T029, T030 |
| FR-011 exactly at the limit accepted | T027, T028 |
| FR-012 stop work, stay healthy, keep serving | T028, T031, T032 |
| FR-013 stream refused, no completion in its place | T038, T039, T040 |
| FR-014 one validated value reaches application logic | T007, T009, T017, T042 |
| FR-015 deterministic, one response, documented precedence | T023, T032, T035, T042 |
| FR-016 precedence order | T007, T028, T035, T040, T042 |
| FR-017 exactly code and message, no details | T008, T029, T048 |
| FR-018 no value, prompt, credential, or parser echo | T023, T028, T029, T039 |
| FR-019 payload_too_large code and message | T028, T029 |
| FR-020 unknown fields ignored | T019, T022, T026 |
| FR-021 no external service during validation | T047 |
| FR-022 concurrent validation isolated | T026, T036 |
| FR-023 existing error contract unchanged | T008, T029, T041, T048 |
| FR-024 liveness, readiness, shutdown unchanged | T032, T037, T049 |
| FR-025 regression and quality checks pass | T001, T049 |
| FR-026 automated coverage of rules, responses, precedence, boundary | T027, T028, T033, T034, T035 |
| FR-027 user-facing documentation | T043, T044, T045, T046, T048 |

| Success criterion | Tasks |
|-------------------|-------|
| SC-001 predictable from documentation | T043, T044, T045 |
| SC-002 100% of rules covered automatically | T033, T034 |
| SC-003 determinism across 100 repetitions | T014, T036 |
| SC-004 boundary correctness | T012, T021, T027, T028 |
| SC-005 fixed responses, zero leakage | T023, T028, T029, T039 |
| SC-006 stable precedence over 20 repetitions | T035 |
| SC-007 only valid requests reach application logic | T007, T009, T033 |
| SC-008 50 concurrent isolated results | T036 |
| SC-009 lifecycle correct throughout | T028, T037 |
| SC-010 existing suite and checks pass | T001, T049 |

---

## Notes

- `[P]` means a different file with no dependency on the other parallel task in the same phase. Two `[P]` tasks in one phase never edit the same file.
- `[Story]` labels map each task to a user story for traceability and to the requirement table above.
- Every user story is independently completable and independently testable; US4 and US5 depend only on Phase 2, not on each other.
- Confirm the new tests fail before implementing, and confirm the existing suite never loses a test.
- Commit after each task or logical group.
- Stop at any checkpoint to validate a story independently.
- The 1 MB limit, the control ranges, and the 14-row error contract are fixed by this feature. Do not make the limit configurable, add a `details` field, or add per-field or per-message content limits — each belongs to a later phase.
