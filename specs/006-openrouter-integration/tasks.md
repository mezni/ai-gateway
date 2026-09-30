# Tasks: OpenRouter Integration

**Input**: Design documents from `/specs/006-openrouter-integration/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: The examples below include test tasks. Tests are OPTIONAL - only include them if explicitly requested in the feature specification.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `- [ ] T### [P?] [Story] Description`

**Format Components**:
1. **Checkbox**: ALWAYS start with `- [ ]` (markdown checkbox)
2. **Task ID**: Sequential number (T001, T002, T003...) in execution order
3. **[P] marker**: Include ONLY if task is parallelizable (different files, no dependencies on incomplete tasks)
4. **[Story] label**: REQUIRED for user story phase tasks only
   - Format: [US1], [US2], [US3], etc. (maps to user stories from spec.md)
   - Setup phase: NO story label
   - Foundational phase: NO story label
   - User Story phases: MUST have story label
   - Polish phase: NO story label
5. **Description**: Clear action with exact file path

**Examples**:
- ✅ CORRECT: `- [ ] T001 Create project structure per implementation plan`
- ✅ CORRECT: `- [ ] T005 [P] Implement authentication middleware in src/middleware/auth.py`
- ✅ CORRECT: `- [ ] T012 [P] [US1] Create User model in src/models/user.py`
- ✅ CORRECT: `- [ ] T014 [US1] Implement UserService in src/services/user_service.py`
- ❌ WRONG: `- [ ] Create User model` (missing ID and Story label)
- ❌ WRONG: `T001 [US1] Create model` (missing checkbox)
- ❌ WRONG: `- [ ] [US1] Create User model` (missing Task ID)
- ❌ WRONG: `- [ ] T001 [US1] Create model` (missing file path)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Create OpenRouter provider module in `src/infrastructure/providers/openrouter.rs` (adapter impl per 005 provider contract; layout adapted from assumed `src/providers/` to repo's `src/infrastructure/providers/`)
- [X] T002 Initialize Rust project with Reqwest dependency (`reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }`) and verify `cargo build` passes
- [X] T003 [P] Configure linting and formatting (cargo fmt --check passes; cargo clippy should be verified — no new warnings introduced by this feature)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Configure OpenRouter provider in `config/gateway.yaml` per plan.md Phase 5 specification (provider config realized through environment variables per 005 contract FR-028; YAML file serves as operator-facing reference documenting default env-var values)
- [ ] T005 [P] Implement LlmProvider trait method for OpenRouter in `src/application/providers.rs`
- [ ] T006 [P] Setup API client configuration with base URL `https://openrouter.ai/api/v1`
- [ ] T007 [P] Implement environment variable reading for `OPENROUTER_API_KEY`
- [ ] T008 [P] Configure request timeout (60s) and connect timeout (30s) for API client
- [ ] T009 [P] Setup error normalization to gateway's flat two-key contract `{"code","message"}`
- [ ] T010 [P] Configure logging to prohibit API key/authorization header output
- [ ] T011 [P] Setup telemetry signal emission structure (request_id, provider, model, latency, status, error_type, attempt, request_count)

---

## Phase 3: User Story 1 - Connect to OpenRouter LLM (Priority: P1) 🎯 MVP

**Goal**: Gateway can successfully send chat completion requests to OpenRouter and receive valid responses using a valid API key

**Independent Test**: The gateway can send a chat completion request to OpenRouter with a valid API key and receive a valid response; or return appropriate error for missing/invalid API key

### Tests for User Story 1 (OPTIONAL - only if tests requested) ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T012 [P] [US1] Contract test for OpenRouter chat completion in `tests/contract/openrouter_chat_completion.py`
- [ ] T013 [P] [US1] Integration test for OpenRouter request/response flow in `tests/integration/openrouter_request_test.rs`

### Implementation for User Story 1

- [ ] T014 [P] [US1] Create OpenRouter request mapper from gateway ChatRequest to OpenAPI format in `src/providers/openrouter_mapper.rs`
- [ ] T015 [P] [US1] Create OpenRouter response mapper from OpenAPI format to gateway ChatResponse in `src/providers/openrouter_mapper.rs`
- [ ] T016 [US1] Implement OpenRouter API call using Reqwest client in `src/providers/openrouter_provider.rs`
- [ ] T017 [US1] Add validation and error handling for API responses
- [ ] T018 [US1] Map OpenRouter errors to gateway's flat two-key contract `{"code","message"}` with normalized codes
- [ ] T019 [US1] Emit telemetry signals: request_id, provider, model, latency, status, error_type, attempt
- [ ] T020 [US1] Mask prompts in telemetry output; prohibit API key logging
- [ ] T021 [US1] Support two user roles: "user" (chat requestors) and "admin" (operational management) in provider config

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T022 [P] Documentation updates in `docs/` - OpenRouter provider integration guide
- [ ] T023 Code cleanup and refactoring - ensure provider abstraction is clean and testable
- [ ] T024 [P] Performance optimization across OpenRouter requests (target: 95% within 5 seconds)
- [ ] T025 [P] Security hardening - verify no API keys or secrets in logs/telemetry
- [ ] T026 [P] Run quickstart.md validation - end-to-end test scenarios
- [ ] T027 [P] Run research.md validation - verify all researched decisions are implemented
- [ ] T028 [P] Run data-model.md validation - verify entity mappings are correct
- [ ] T029 [P] Run contracts.md validation - verify API contract mappings are correct
- [ ] T030 Run full cargo clippy `-- -D warnings` and cargo build `--release`