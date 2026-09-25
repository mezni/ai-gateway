# Data Model: Project Foundation

**Feature**: [spec.md](spec.md)
**Date**: 2026-09-24

The foundation phase introduces **no persistent runtime data**. The entities
below are the *development artifacts* (design-time entities) produced by this
phase and consumed by every later phase. They are documented here so later
phases extend them consistently.

## Entities

### Project Manifest (`Cargo.toml`)

Represents the build configuration and identity of the gateway project.

| Attribute  | Type      | Rules                                                                 |
|------------|-----------|-----------------------------------------------------------------------|
| name       | string    | `ai-gateway`; matches crate/package identity                          |
| version    | semver    | initial `0.1.0`                                                       |
| edition    | string    | MUST be `2024` (implies resolver = "3")                               |
| dependencies | map     | MUST be empty (zero runtime dependencies, FR-011)                     |
| lints      | table     | optional baseline lint levels (Clippy/rustfmt)                        |

### Toolchain Pin (`rust-toolchain.toml`)

Locks the Rust toolchain for local and CI reproducibility.

| Attribute  | Type      | Rules                                                                 |
|------------|-----------|-----------------------------------------------------------------------|
| channel    | string    | MUST match resolved stable channel (1.98.1)                           |
| profile    | string    | `minimal`                                                             |
| components | list      | includes `clippy`, `rustfmt`                                          |

### Entry Point (`src/main.rs`, `src/lib.rs`)

The runnable application and its testable logic core.

| Attribute | Type | Rules |
|-----------|------|-------|
| application | binary | MUST build and run; `main` delegates to lib |
| library    | lib   | exposed crate root; carries the minimal logic and unit tests |

Validation: FR-002 (entry point runs), FR-003 (compiles end to end),
FR-004 (test suite passes).

### Ignore Rules (`.gitignore`)

Controls what must never be committed.

| Included rule scheme | Examples |
|----------------------|----------|
| Build output         | `/target`, `**/*.rs.bk`, `*.pdb` |
| Local environment    | `.env`, `*.local` |
| Editor/OS artifacts  | `.DS_Store`, editor dirs, `rustc-ice-*.txt` |

Validation: FR-008, SC-004 (no generated/secrets/machine files in VCS).

### Environment Template (`.env.example`)

The contract between externalized configuration and actual configuration.

| Attribute  | Type   | Rules                                                            |
|------------|--------|------------------------------------------------------------------|
| namespace  | string | `AI_GATEWAY_` reserved prefix for all gateway variables          |
| casing     | string | `SCREAMING_SNAKE_CASE`                                           |
| values     | tokens | placeholders only (`<...>`); NEVER real values (FR-007, FR-012)  |
| read-set   | list   | empty in this phase (no variable is read yet)                    |

Validation: FR-007, FR-012, SC-004.

### Readme (`README.md`)

The onboarding artifact for developers.

| Attribute | Type | Rules |
|-----------|------|-------|
| purpose    | text | project purpose and phase scope |
| prerequisites | text | toolchain channel and install pointer |
| setup      | text | clone + env template steps |
| commands   | text | check, test, format, lint, run |

Validation: FR-009, FR-010, SC-001/SC-003.

### Quality Gate (workflow contract)

The set of checks gating acceptance; see [contracts/quality-gates.md](contracts/quality-gates.md).

| Gate    | Pass condition                                   |
|---------|--------------------------------------------------|
| build   | `cargo check` and release build succeed          |
| test    | `cargo test` all tests pass                      |
| format  | `cargo fmt --all --check` no diffs               |
| lint    | `cargo clippy --all-targets -- -D warnings` clean |

Validation: FR-003, FR-005, FR-006, SC-002, SC-005.

## Relationships

```text
rust-toolchain.toml  ──pins──►  Cargo.toml
Cargo.toml           ──defines► src/main.rs + src/lib.rs
.gitignore           ──protects► (target, .env, local files)
.env.example         ──sample──► future AI_GATEWAY_* variables
README.md            ──documents► all of the above
Quality gate         ──validates► build + test + format + lint
```

## State Transitions

No runtime state transitions apply (zero runtime data in this phase). The
artifacts are *created* (Phase 0) and subsequently *extended* by later
feature phases (dependencies added to `Cargo.toml`, read-set grown in
`.env.example`, etc.). Each later phase MUST update this data model.

## Validation Rules (mapped from spec requirements)

- FR-003/SC-005: the crate MUST compile and remain runnable after every change.
- FR-004/SC-002: the test suite MUST pass consistently.
- FR-005: formatting MUST be uniform (fmt check clean).
- FR-006: static analysis MUST report zero blocking warnings.
- FR-007/FR-012/SC-004: environment template holds placeholders only; no secrets or generated files in VCS.
- FR-011: dependency set MUST remain empty unless a later spec justifies an addition.