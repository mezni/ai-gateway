# Changelog

All notable changes to the AI Gateway project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project documentation: PRD, Architecture, API, Configuration,
  Providers, Security, and Reliability docs under `docs/`
- README with architecture overview, phased roadmap, and documentation index
- Project structure with `docs/`, `specs/`, `docs/adr/`
- Spec Kit constitution (v1.0.0 ratified, v1.1.0 amended with
  `specs/` artifact requirement) and memory
- `CHANGELOG.md` and `HANDOFF.md`
- Phase 0 project foundation: zero-dependency Rust crate (Edition 2024,
  toolchain pinned via `rust-toolchain.toml`), `src/lib.rs` + `src/main.rs`
  entry point, `.gitignore`, `.env.example`, and README Development section
  with the quality-gate workflow (fmt/clippy/check/test/release)
- Spec Kit feature artifacts for `specs/001-project-foundation` (spec, plan,
  research, data model, contracts, tasks, quickstart)
- Spec Kit planning artifacts for `specs/002-layered-architecture` (spec,
  clarification for essential domain fields, plan, research, data model,
  module boundary contract, quickstart)
- Phase 1 layered architecture: lib-first module skeleton with the five
  layers (`domain`/`application`/`api`/`infrastructure`/`config`),
  provider-independent domain types (`ChatRequest`, `Message`, `MessageRole`,
  `ChatResponse`, `Usage`, `Model`, `Provider`), and an `AppState` composition
  root exercised by `main.rs`

### Changed
- Converted flow/architecture diagrams in markdown docs to Mermaid
- Aligned README target structure with architecture layering and
  YAML-based configuration in `docs/configuration.md`
- Replaced root `constitution.md` scaffold template with the ratified
  constitution text
- Normalized doc titles under `docs/` for consistency

### Removed
- Reference to non-existent `docs/routing.md` and `docs/operations.md` in
  the README target structure

