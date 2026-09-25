# Quickstart / Validation Guide: Project Foundation

**Feature**: [spec.md](spec.md) | **Contracts**: [environment](contracts/environment.md), [quality gates](contracts/quality-gates.md) | **Data model**: [data-model.md](data-model.md)

This guide validates the foundation scaffold end to end. It is a run/verify
guide only — implementation detail lives in `tasks.md`.

## Prerequisites

- Rust toolchain **1.98.1** (pinned by `rust-toolchain.toml`; rustup will
  honor it automatically). Install pointer in README.
- Supported platform: Linux.

## Setup

```bash
git clone <repo-url> ai-gateway
cd ai-gateway
cp .env.example .env   # optional; no variables are read in this phase
```

## Validation Scenarios

### Scenario 1 — Builds and Runs

```bash
cargo check
cargo run --quiet
```

**Expected**: `cargo check` succeeds; the application starts and terminates
cleanly with exit code 0, printing its banner/version line.

### Scenario 2 — Formatting

```bash
cargo fmt --all --check
```

**Expected**: exits 0 with no diff output.

### Scenario 3 — Static Analysis

```bash
cargo clippy --all-targets -- -D warnings
```

**Expected**: zero warnings or errors.

### Scenario 4 — Unit Tests

```bash
cargo test
```

**Expected**: all tests pass (minimal smoke tests in `src/lib.rs`).

### Scenario 5 — Documented Workflow Is Discoverable

Open README and confirm it documents: purpose, prerequisites (toolchain),
setup steps, and the commands above.

**Expected**: a developer can run all quality gates using only the README
(SC-001 / SC-003).

### Scenario 6 — Repository Hygiene

```bash
git status --short
```

**Expected**: no `target/`, `.env`, or local/editor artifacts tracked; no
real secrets anywhere in the tree (SC-004).

## References

- Environment variable contract: [contracts/environment.md](contracts/environment.md)
- Quality gate contract: [contracts/quality-gates.md](contracts/quality-gates.md)
- Artifacts and attributes: [data-model.md](data-model.md)
- Requirements: [spec.md](spec.md) (FR-001..FR-012, SC-001..SC-005)