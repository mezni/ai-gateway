---

description: "Task list for the Project Foundation feature"

---

# Tasks: Project Foundation

**Input**: Design documents from `/specs/001-project-foundation/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: The spec requires a minimal automated test suite (FR-004) and the quickstart validation guide relies on `cargo test` (Scenario 4). Smoke-test tasks are therefore included; they are deliberately minimal and do NOT imply a TDD approach for this phase.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Single binary crate at repository root: `Cargo.toml`, `rust-toolchain.toml`, `src/main.rs`, `src/lib.rs`, `.gitignore`, `.env.example`, `README.md`
- Zero runtime dependencies (standard library only) per FR-011 — do NOT add deps

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization — manifest, toolchain pin, repository hygiene config

- [X] T001 Create Cargo.toml package manifest with name `ai-gateway`, version `0.1.0`, edition `2024`, and empty `[dependencies]` table
- [X] T002 [P] Create rust-toolchain.toml pinning `channel = "1.98.1"`, `profile = "minimal"`, `components = ["clippy", "rustfmt"]`
- [X] T003 [P] Create .gitignore with `/target`, `.env`, `*.local`, and editor/OS artifact entries (see data-model.md ignore rules)
- [X] T004 [P] Create .env.example with `AI_GATEWAY_` documented placeholder template and comment header (see contracts/environment.md)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Minimal runnable crate skeleton that MUST exist before ANY user story can be verified

**⚠️ CRITICAL**: No user story work can begin until the crate builds and runs

- [X] T005 Create src/lib.rs exposing a minimal public `app_version()` function returning the crate version string
- [X] T006 Create src/main.rs as a thin runner that prints the gateway banner using `app_version()` and exits with code 0

**Checkpoint**: `cargo check` succeeds and `cargo run --quiet` prints the banner — foundation ready for user stories

---

## Phase 3: User Story 1 - Establish a Clean, Working Project Scaffold (Priority: P1) 🎯 MVP

**Goal**: A fresh developer-facing, zero-dependency Rust crate that builds and runs, with all foundation artifacts present and referenced in the readme.

**Independent Test**: Locate all foundation artifacts (Cargo.toml, rust-toolchain.toml, src/main.rs, src/lib.rs, .gitignore, .env.example, README.md); run `cargo check` then `cargo run`; confirm the application produces its banner and terminates cleanly (exit 0).

### Implementation for User Story 1

- [X] T007 [P] [US1] Complete Cargo.toml [package] metadata (description, repository, license) aligned with README purpose and empty-dependency policy
- [X] T008 [US1] Ensure `rust-toolchain.toml` is honored by rustup (`rustc --version` reports pinned 1.98.1)
- [X] T009 [US1] Verify the scaffold end to end: `cargo check` and `cargo run --quiet` both succeed and the application exits 0

**Checkpoint**: At this point User Story 1 is runnable on its own.

---

## Phase 4: User Story 2 - Run Quality Checks That All Pass (Priority: P1) 🎯 MVP

**Goal**: The documented quality gates (format, lint, tests, build, release build) all pass consistently.

**Independent Test**: From a clean checkout, run each gate command in the order defined in contracts/quality-gates.md and confirm all succeed with no output beyond normal completion.

### Tests for User Story 2

- [X] T010 [P] [US2] Add smoke unit test for `app_version()` in src/lib.rs under `#[cfg(test)]` asserting the returned string matches the Cargo.toml version

### Implementation for User Story 2

- [X] T011 [US2] Make all quality gates pass: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check`, `cargo test`, `cargo build --release`
- [X] T012 [US2] Resolve any formatting/lint findings so gate #2 reports zero warnings (`-D warnings` clean)

**Checkpoint**: User Stories 1 AND 2 both work independently; definition-of-done gates are green.

---

## Phase 5: User Story 3 - Onboard a New Developer (Priority: P2)

**Goal**: A developer unfamiliar with the project can clone, set up, and run the full workflow using only the README.

**Independent Test**: Hand the repository to a new developer; without external help they copy `.env.example` to `.env`, run the documented commands, and reach a passing checkout.

### Implementation for User Story 3

- [X] T013 [P] [US3] Add a "Development" section to README.md documenting prerequisites (Rust 1.98.1 via rust-toolchain.toml), setup (`cp .env.example .env`), and the standard command workflow (check/test/fmt/clippy/run)
- [X] T014 [US3] Add environment-var documentation section to README.md linking the `AI_GATEWAY_` template contract and stating no variables are read in this phase
- [X] T015 [US3] Verify the onboarding flow per quickstart.md Scenario 5: all commands listed in README execute successfully from a clean clone

**Checkpoint**: All user stories are independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Verification and hygiene affecting the whole foundation

- [X] T016 [P] Run all six quickstart.md validation scenarios end to end and record results
- [X] T017 [P] Verify repository hygiene: `git status` shows no `target/`, `.env`, or local/editor artifacts; no real secrets in any tracked file
- [X] T018 Update docs/plan.md and CHANGELOG.md with the completed Phase 0 / foundation milestone status

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - US1 and US2 are both P1 and can run in parallel after Phase 2
  - US3 depends on artifact content from US1/US2 (README references working commands)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Depends on T001–T006. No dependencies on other stories.
- **User Story 2 (P1)**: Depends on T005/T006 (lib + main must exist to test). Independent of US1's metadata work.
- **User Story 3 (P2)**: Depends on US1 and US2 being complete (README documents verified commands and green gates).

### Within Each User Story

- Tests (T010) before gate validation (T011/T012)
- Library core before binary entry point (T005 → T006)
- Metadata and verification before story sign-off (T007/T008 → T009)

### Parallel Opportunities

- T002, T003, T004 run in parallel with T001
- T005 and T006 are sequential (main depends on lib)
- T007, T008, T010 are file-independent and can run in parallel
- US1 (T007–T009) and US2 (T010–T012) can run in parallel after Phase 2
- T013 and T014 (README sections) can run in parallel; both PR-able to README.md
- T016 and T017 can run in parallel in the polish phase

---

## Parallel Example: User Story 1 + User Story 2

```bash
# Launch Setup + Foundational tasks in parallel:
Task: "Create Cargo.toml package manifest..."
Task: "Create rust-toolchain.toml pinning..."
Task: "Create .gitignore..."
Task: "Create .env.example..."

# Then User Story 1 and 2 in parallel:
Task: "Complete Cargo.toml [package] metadata..."     # US1
Task: "Add smoke unit test for app_version()..."      # US2
```

---

## Implementation Strategy

### MVP First (User Story 1 + User Story 2)

Both P1 stories form the MVP because the constitution's definition of done
requires the quality gates (build, test, format, lint) to pass:

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks everything)
3. Complete Phase 3: User Story 1
4. Complete Phase 4: User Story 2 (gates green)
5. **STOP and VALIDATE**: run quickstart.md Scenarios 1–4
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → crate builds and runs
2. Add User Story 1 → scaffold verified (runnable, artifacts present)
3. Add User Story 2 → all quality gates pass (MVP!)
4. Add User Story 3 → onboarding documented and verified
5. Polish: full quickstart run + hygiene audit

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
3. Developer C: User Story 3 (after US1/US2 green)
4. Polish phase by any team member

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Zero-dependency policy (FR-011): `cargo add` is NOT permitted in this phase
- Commit `Cargo.lock` once generated (binary crate reproducible builds)
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently