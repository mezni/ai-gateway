# Research: Project Foundation

**Feature**: [spec.md](spec.md) — Phase 0 of the AI Gateway roadmap.
**Date**: 2026-09-24
**Status**: All Technical Context unknowns resolved.

## 1. Toolchain & Edition

- **Decision**: Target Rust 1.98.1 (current stable, 2026-09) with Edition 2024, pinned via a `rust-toolchain.toml` file (profile `minimal`, components `clippy` + `rustfmt`).
- **Rationale**: Edition 2024 is the current edition and the default for new projects (`cargo new`), and implies the MSRV-aware `resolver = "3"`. Pinning via `rust-toolchain.toml` (the recommended, forward-compatible file name) makes local and CI toolchains deterministic. Applications should track latest stable (only latest stable receives security fixes) rather than declare a legacy MSRV.
- **Alternatives considered**:
  - Defer to "latest stable, unpinned" — rejected: non-reproducible builds and CI drift.
  - Legacy `rust-toolchain` file — rejected: deprecated in favor of `rust-toolchain.toml`.
  - Edition 2021 — rejected: 2024 is current and default for new projects.

## 2. Runtime Dependencies

- **Decision**: Zero runtime dependencies for the foundation scaffold (standard library only). `[dependencies]` stays empty.
- **Rationale**: Canonical guidance is to add crates only when a feature requires them, keeping compile time, binary size, and attack surface minimal, and honoring the incremental architecture principle. Unused dependencies are also caught by lint tooling.
- **Alternatives considered**:
  - Bootstrap with `tokio` + `tracing` — rejected: no async or logging feature exists in this phase; each belongs to its own later specification.
  - Add `anyhow`/`thiserror` — rejected: no nontrivial error surface exists yet.

## 3. Lint & Formatting Configuration

- **Decision**: Enforce checks at the workflow/CI boundary with `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings`; optionally encode baseline lint levels via the `[lints]` table in `Cargo.toml`.
- **Rationale**: `-D warnings` makes lint breach immediately visible and is the project-wide standard (constitution Definition of Done). The `[lints]` table is the modern stable way to version lint levels with the manifest.
- **Alternatives considered**: `clippy.toml`/`clippy-driver` config — rejected for foundation (no clippy configuration values needed yet). External lint policy files return when later phases introduce CI.

## 4. Version Control Policy

- **Decision**: Commit `Cargo.lock` (binary crate), ignore `target/`, environment files, and local/editor artifacts.
- **Rationale**: For binaries, `Cargo.lock` guarantees reproducible dependency resolution across machines and CI. Generated output and local configuration must never enter the repository.
- **Alternatives considered**: Ignore `Cargo.lock` (library convention) — rejected: this is an application producing a runnable binary.

## 5. Environment Variable Template

- **Decision**: Provide root-level `.env.example` using `SCREAMING_SNAKE_CASE`, reserved `AI_GATEWAY_` namespace, placeholder values only, with per-variable comments documenting required/optional, default, and purpose.
- **Rationale**: Externalized configuration is a constitution requirement; the template establishes the naming and documentation contract. In this phase the application reads no environment variables yet, so the file is a documented template for upcoming phases.
- **Alternatives considered**: Committing a real `.env` — rejected (secret leakage, FR-012). Leaving out the template — rejected (FR-007).

## 6. README Structure

- **Decision**: README documents purpose, prerequisites (Rust 1.98.1), setup (clone + env template), and the full development command workflow (check/test/fmt/clippy), plus the roadmap link.
- **Rationale**: FR-009 requires setup and command documentation discoverable without tribal knowledge; the existing top-level README is a product/architecture document and will gain a "Development" section (or pointer to this spec kit artifact) rather than duplicating details.
- **Alternatives considered**: Embedding full spec-kit details inline — rejected: keep README lean and point to `specs/` and `docs/`.

## 7. Quality Gates

- **Decision**: The foundation workflow is, in order: `cargo fmt --all --check` → `cargo clippy --all-targets -- -D warnings` → `cargo check` → `cargo test` → `cargo run` (smoke).
- **Rationale**: Delivers the definition-of-done gates (format, lint, tests, build) from the very first commit; CI automation for these is added in the CI/CD phase but the commands are fixed here so local and CI stay identical.
- **Alternatives considered**: Adding `cargo audit`/`cargo deny` dependency scanning now — deferred to the CI/CD phase; there are no dependencies to scan in this phase.

## 8. Platform

- **Decision**: Primary target is Linux (dev + CI); behavior on other platforms is best-effort and documented, never a release gate.
- **Rationale**: Phase scope and repo docs (Docker later) assume a Linux server payload; constraining the published support contract keeps acceptance testing deterministic.
- **Alternatives considered**: Full cross-platform matrix — rejected as out of scope for a foundation scaffold.