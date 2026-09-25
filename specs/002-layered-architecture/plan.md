# Implementation Plan: Layered Architecture

**Branch**: `002-layered-architecture` | **Date**: 2026-09-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-layered-architecture/spec.md`

**Note**: This plan was filled by the `/speckit.plan` command.

## Summary

Translate the AI Gateway architecture into enforceable, compilable module
boundaries. Introduce the layered skeleton (`domain`, `application`,
`infrastructure`, `api`, `config`) using idiomatic Rust module layout, define
the initial provider-independent domain types (chat request, message, message
role, chat response, usage, model, provider) with essential fields only, and
establish a framework-free composition seam (`AppState`) — all with zero new
dependencies and the application still runnable.

## Technical Context

**Language/Version**: Rust 1.98.1 / Edition 2024 (pinned by `rust-toolchain.toml`). Resolved from Phase 0 research.

**Primary Dependencies**: None. The layered skeleton and domain types use the standard library only; no new crates are introduced (FR-005, FR-011).

**Storage**: N/A — no persistence in this phase.

**Testing**: `cargo test`. Unit tests exercise every domain type (construction, role variants, usage accounting); the existing `app_version()` test stays.

**Target Platform**: Linux (unchanged).

**Project Type**: Binary crate with a lib-first split. All logic lives in the lib crate (`src/lib.rs`); `main.rs` is a thin runner that constructs the composition root and prints the banner.

**Performance Goals**: N/A — no workload exists in this phase.

**Constraints**: Inward-only cross-layer dependencies (`api → application → domain`, `infrastructure → domain`); domain must not import application/infrastructure/api/config; no provider concepts in the domain model; quality gates remain green.

**Scale/Scope**: One lib module tree: `domain/`, `application`, `api`, `infrastructure`, `config`, plus wiring in `main`. Explicitly out of scope: validation, provider mapping, HTTP, routing, persistence.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Constitution Principle | Status | Assessment |
|---|---|---|
| II. Rust-First Engineering | PASS | Idiomatic Edition 2024 module layout (file-stem pattern), lib-first split. |
| III. Incremental Architecture | PASS | Module skeleton and domain types only; no premature infrastructure. |
| IV. Provider Independence | PASS | Domain types carry no provider-specific concepts; infrastructure layer is empty. |
| V. Explicit Boundaries | PASS | Distinct `domain/application/infrastructure/api/config` modules with inward-only dependency rules. |
| VI. Reliability & Failure Isolation | PASS (N/A scope) | No external calls this phase; application remains runnable. |
| VII. Security & Privacy by Design | PASS (N/A scope) | No credentials, no external input; secrets handling deferred. |
| VIII. Observability | PASS (N/A scope) | Deferred to observability phase. |
| IX. Testability & Correctness | PASS | Every domain type is exercised by unit tests; regression suite preserved. |
| X. Performance & Resource Discipline | PASS | Zero new dependencies; `Arc`-based state sharing only when needed. |
| Engineering Standard: Configuration | PASS | `config` module prepared; no config values yet. |
| Engineering Standard: Documentation | PASS | Module boundary contract (`contracts/layout.md`) + plan/data-model/quickstart persisted. |

**Gate result**: PASS — no violations require complexity justification.

## Project Structure

### Documentation (this feature)

```text
specs/002-layered-architecture/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── layout.md
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── lib.rs              # Crate root: declares modules, keeps app_version()
├── main.rs             # Thin runner: builds AppState, prints banner
├── domain.rs           # Domain module root: re-exports chat + catalog
├── domain/
│   ├── chat.rs         # ChatRequest, Message, MessageRole, ChatResponse, Usage (+ tests)
│   └── catalog.rs      # Model, Provider (+ tests)
├── application.rs      # Application layer root: AppState composition root
├── api.rs              # API layer root (placeholder for future phases)
├── infrastructure.rs   # Infrastructure layer root (placeholder for future phases)
└── config.rs           # Configuration layer root (placeholder for future phases)
```

**Structure Decision**: File-stem module layout (Edition 2024 convention —
`src/domain.rs` + `src/domain/chat.rs`, not `mod.rs`). Lib-first split keeps
everything integration-testable via `use ai_gateway::...`. Dependency rules
point inward only; `domain` depends on nothing.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

N/A — no violations. This is the simplest structure satisfying the phase
requirements; layers are empty except where the phase requires content.