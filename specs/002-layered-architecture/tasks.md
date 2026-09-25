---

description: "Task list for the Layered Architecture feature"

---

# Tasks: Layered Architecture

**Input**: Design documents from `/specs/002-layered-architecture/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/layout.md

**Tests**: Unit tests are included because FR-002/SC-002 require every domain type to be exercised by the test suite with no external service. They are implementation-scaffold tests, not a separate TDD workflow.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Lib-first crate: logic in `src/` modules; `main.rs` is a thin runner
- File-stem module layout: `src/domain.rs` + `src/domain/chat.rs`, **not** `mod.rs`
- Zero new dependencies — standard library only

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module tree skeleton all stories depend on

- [X] T001 Declare the layered module tree in src/lib.rs (`pub mod domain; pub mod application; pub mod api; pub mod infrastructure; pub mod config;`) while preserving the existing `app_version()` function
- [X] T002 [P] Create placeholder modules src/application.rs, src/api.rs, src/infrastructure.rs, src/config.rs, each with a doc comment stating its ownership per contracts/layout.md
- [X] T003 Create src/domain.rs module root with a doc comment describing the domain module (submodule declarations added in US2)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Validate the skeleton compiles before any story work proceeds

**⚠️ CRITICAL**: No user story work can begin until the module tree compiles

- [X] T004 Run `cargo check` and confirm the layered module skeleton compiles cleanly (foundational checkpoint)

**Checkpoint**: Architecture skeleton foundation ready — user story implementation can begin

---

## Phase 3: User Story 1 - Establish Clear Architectural Boundaries (Priority: P1) 🎯 MVP

**Goal**: The layered boundaries exist, compile, and are documented so future features have an unambiguous home.

**Independent Test**: Inspect the module tree, confirm each planned layer exists, run `cargo check`, and confirm the boundary contract (contracts/layout.md) is documented and holds.

### Implementation for User Story 1

- [X] T005 [P] [US1] Add a "Project Structure" section to README.md describing the layered modules and their ownership, mirroring contracts/layout.md (dependency direction: api → application → domain; infrastructure → domain)

**Checkpoint**: Boundary structure is in place and navigable.

---

## Phase 4: User Story 2 - Define the Initial Domain Model (Priority: P1) 🎯 MVP

**Goal**: The seven provider-independent domain types exist with essential fields and are exercised by tests.

**Independent Test**: Confirm `ChatRequest`, `Message`, `MessageRole`, `ChatResponse`, `Usage`, `Model`, `Provider` all exist in src/domain/, compile, and pass their unit tests with no external service.

### Tests for User Story 2

- [X] T006 [P] [US2] Add unit tests in src/domain/chat.rs covering ChatRequest construction (model + messages), MessageRole variants (System/User/Assistant), and Usage accounting (total equals prompt + completion)
- [X] T007 [P] [US2] Add unit tests in src/domain/catalog.rs covering Model and Provider construction with essential fields

### Implementation for User Story 2

- [X] T008 [P] [US2] Create src/domain/chat.rs defining ChatRequest (model, messages), Message (role, content), MessageRole enum (System, User, Assistant), ChatResponse (content, usage), Usage (prompt_tokens, completion_tokens, total_tokens with a constructor enforcing total = prompt + completion)
- [X] T009 [P] [US2] Create src/domain/catalog.rs defining Model (id) and Provider (id, name) as plain value types
- [X] T010 [US2] Wire src/domain.rs to declare `pub mod chat; pub mod catalog;` and re-export the public types
- [X] T011 [US2] Verify provider independence: grep src/domain/ for provider-specific terms (e.g., openrouter, api-key, provider url) and confirm none are present

**Checkpoint**: Domain model is defined, compiled, and independently testable.

---

## Phase 5: User Story 3 - Prepare Composition Without Coupling (Priority: P2)

**Goal**: An explicit composition mechanism exists and the application runs with no real provider dependency.

**Independent Test**: Run the application and confirm the banner flows through a composition root constructed in main; run with no provider configured and confirm clean start/exit; remove any placeholder component and confirm the build still succeeds.

### Implementation for User Story 3

- [X] T012 [P] [US3] Implement an AppState composition root in src/application.rs that holds the application version and exposes a `banner()` method (constructor-injected dependency, per research.md)
- [X] T013 [US3] Refactor src/main.rs into a thin composition root that constructs `AppState::new(...)` and prints the banner via the state (preserving the exact banner output `AI Gateway v0.1.0`)

**Checkpoint**: Application composes cleanly and remains runnable without providers.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Full validation, regression gates, and documentation

- [X] T014 [P] Run quickstart.md Scenarios 1-6 end to end and record results
- [X] T015 [P] Verify all quality gates pass: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`
- [X] T016 Update docs/plan.md Phase 1 status to COMPLETE and add a CHANGELOG.md entry for the layered architecture milestone

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational
  - US1, US2, US3 are mutually independent after Phase 2; US3's main.rs task depends only on its own AppState task
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: T001-T004 → T005
- **User Story 2 (P1)**: T001-T004 → T006/T007 (tests) → T008/T009 → T010 → T011
- **User Story 3 (P2)**: T001-T004 → T012 → T013
- US3 is independent of US2 (banner does not depend on domain types this phase)

### Within Each User Story

- Tests before implementation for US2 (T006/T007 written before or with T008/T009)
- Implementations (T008/T009) before wiring (T010)
- AppState before main.rs refactor within US3

### Parallel Opportunities

- T002, T003 run in parallel with T001
- US1 (T005), US2 (T006-T011), US3 (T012-T013) can all run in parallel after Phase 2
- T006 and T007 (US2 tests) run in parallel
- T008 and T009 (US2 types) run in parallel
- T014 and T015 run in parallel in polish

---

## Parallel Example: User Story 2

```bash
# Launch tests + implementations together:
Task: "Add unit tests in src/domain/chat.rs covering ChatRequest..."
Task: "Add unit tests in src/domain/catalog.rs covering Model..."
Task: "Create src/domain/chat.rs defining ChatRequest..."
Task: "Create src/domain/catalog.rs defining Model..."
```

---

## Implementation Strategy

### MVP First (User Story 1 + User Story 2)

Both P1 stories form the MVP (structure + domain model). Order:

1. Complete Phase 1: Setup (module tree)
2. Complete Phase 2: Foundational (compile checkpoint)
3. Complete Phase 3: User Story 1 (documented boundaries)
4. Complete Phase 4: User Story 2 (domain types + tests)
5. **STOP and VALIDATE**: run quickstart.md Scenarios 1-2
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → layered skeleton compiles
2. Add User Story 1 → boundaries documented and verified
3. Add User Story 2 → domain model defined and tested (MVP!)
4. Add User Story 3 → composition without coupling proven
5. Polish: quickstart run + regression gates + docs

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
   - Developer C: User Story 3
3. Polish phase by any team member

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Zero-dependency policy (FR-011): `cargo add` is NOT permitted in this phase
- Do NOT use `mod.rs`; use the file-stem module layout (research.md)
- The `app_version()` public API in src/lib.rs must be preserved (crate regression)
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently