# Quickstart / Validation Guide: Layered Architecture

**Feature**: [spec.md](spec.md) | **Contracts**: [layout](contracts/layout.md) | **Data model**: [data-model.md](data-model.md)

This guide validates the architecture skeleton and domain model end to end.
Implementation detail lives in `tasks.md`.

## Prerequisites

- Rust toolchain 1.98.1 (pinned by `rust-toolchain.toml`).
- Phase 0 foundation (present): crate builds/runs, gates configured.

## Setup

```bash
cd ai-gateway   # existing checkout/directory
```

## Validation Scenarios

### Scenario 1 — Layers Exist and Compile

```bash
cargo check
```

**Expected**: compiles cleanly. The module tree contains `domain/`
(`chat.rs`, `catalog.rs`), `application.rs`, `api.rs`, `infrastructure.rs`,
`config.rs` per [contracts/layout.md](contracts/layout.md).

### Scenario 2 — Domain Types Are Exercised

```bash
cargo test
```

**Expected**: all tests pass, including the new domain type tests (chat
request construction, message roles, usage accounting, model/provider
construction) and the retained foundation test.

### Scenario 3 — No Provider Dependency

```bash
grep -rEi 'openrouter|api[-_]?key|provider[_-]?url' src/domain || echo "no provider concepts in domain"
cargo run --quiet
```

**Expected**: no provider concepts in the domain layer; the application
starts, prints its banner, and exits 0 with no credentials configured.

### Scenario 4 — Composition Seam Works

Run the application and confirm the banner is produced through the composition
root (`main` → `AppState`), not hard-coded in `main`.

**Expected**: `cargo run --quiet` prints `AI Gateway v0.1.0` and exits 0.

### Scenario 5 — Regression Green

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo build --release
```

**Expected**: all gates pass (SC-005).

### Scenario 6 — Boundary Contract Holds

Inspect `src/` imports: no `domain` file imports `application`/`api`/
`infrastructure`/`config`.

**Expected**: dependency rules from [contracts/layout.md](contracts/layout.md)
hold (SC-006 — documented placement).

## References

- Module boundary contract: [contracts/layout.md](contracts/layout.md)
- Entities and field rules: [data-model.md](data-model.md)
- Requirements: [spec.md](spec.md) (FR-001..FR-010, SC-001..SC-006)