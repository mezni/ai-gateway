# Quality Gate Contract

**Feature**: [Project Foundation](../spec.md)
**Date**: 2026-09-24

## Purpose

Defines the development workflow checks that MUST pass before any change is
accepted into the AI Gateway repository. These are the definition-of-done
gates for foundation and every later phase.

## Gates

Run in this order from a clean checkout:

| # | Check | Command | Pass condition |
|---|-------|---------|----------------|
| 1 | Format | `cargo fmt --all --check` | No diff reported |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` | Zero warnings/errors |
| 3 | Compile | `cargo check` | Succeeds |
| 4 | Tests | `cargo test` | All tests pass |
| 5 | Release build | `cargo build --release` | Succeeds |
| 6 | Smoke run | `cargo run --quiet` | Application starts and exits 0 |

## Failure Behavior

- A failing gate MUST block the change from merging.
- Check output MUST make the failure identifiable (file/line or test name).
- There is no manual bypass; a contributor fixes the violation and re-runs
  the full gate sequence.

## Scope Notes

- `cargo audit`/`cargo deny` dependency scanning is introduced by the CI/CD
  phase once dependencies exist.
- CI automation is a CI/CD-phase task; these commands here define the local
  workflow contract that CI will mirror.

## Verification (maps to success criteria)

- SC-002: gates pass consistently on repeated clean-environment runs.
- SC-005: the project remains runnable after every change (gates enforced
  from the first commit).