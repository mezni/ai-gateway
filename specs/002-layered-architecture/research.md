# Research: Layered Architecture

**Feature**: [spec.md](spec.md) — Phase 1 of the AI Gateway roadmap.
**Date**: 2026-09-24
**Status**: All Technical Context unknowns resolved (remaining context carried forward from Phase 0 research).

## 1. Rust Module Layout (Edition 2024)

- **Decision**: Use the file-stem pattern — `src/domain.rs` plus `src/domain/chat.rs` submodule files — instead of `mod.rs` directories.
- **Rationale**: The file-stem pattern is the stable modern convention (since Rust 1.30), avoids many identically named `mod.rs` files that confuse editor navigation, and is what Edition 2024 tooling generates.
- **Alternatives considered**: `mod.rs` directories — rejected as legacy convention.
- Source: Rust Reference modules; Rust Book ch. 7; Google Comprehensive Rust.

## 2. Lib-first Split and Tests

- **Decision**: Keep all logic in the lib crate (`src/lib.rs`); `main.rs` stays a thin runner; integration tests import via the crate name (`use ai_gateway::domain::chat::ChatRequest;`).
- **Rationale**: Binary crates cannot be integration-tested directly; the lib-first split is the standard fix and keeps the existing `app_version()` API stable.
- **Alternatives considered**: Logic directly in the binary crate — rejected (untestable from integration tests, couples entry point to domain).
- Source: Cargo project layout; Rust Book ch. 11.

## 3. Composition Without a Framework

- **Decision**: A single `AppState` composition root holds application-level dependencies, constructed once in `main`, shared by `Arc` when needed. Fields are injected via constructors (`AppState::new(...)`).
- **Rationale**: This is the idiomatic minimal plain-Rust DI pattern — no container, compile-time checked, testable with future trait-object swaps. Satisfies FR-006 with zero dependencies.
- **Alternatives considered**: Trait-object injection now — deferred until a real component (provider) exists. Generic-bounds injection — deferred likewise. A DI container/`injectable` crate — rejected (adds dependency, Phase 0 zero-dep policy).
- Source: Rust web/state-management references on `AppState` composition; Rust DI patterns via traits/constructors.

## 4. Domain Module Ownership

- **Decision**: `domain` groups chat concepts (`chat.rs`) and catalog concepts — model/provider (`catalog.rs`) — with essential fields only; domain re-exports them as a public API surface.
- **Rationale**: Keeps the domain module small but meaningful, mirrors the spec's entity list, and follows the clarified attribute depth (Option A from clarify session).
- **Alternatives considered**: One flat `domain.rs` — fine for now but less navigable as domain grows. Empty placeholder types — rejected (conflicts with FR-004).
- Source: layered/hexagonal Rust references.

## 5. Cross-Layer Dependency Rules

- **Decision**: Inward-only dependencies — `api → application → domain`, `infrastructure → domain`; the domain layer depends on nothing; there are no known forward (inward) references.
- **Rationale**: This is the standard rule for layered/hexagonal Rust applications and directly implements constitution Principle V.
- **Alternatives considered**: Flat, unrestricted imports — rejected (would defeat the skeleton's purpose).
- Source: layered/hexagonal architecture references for Rust.

## 6. Application State / Composition Root Location

- **Decision**: `AppState` lives in the `application` layer root (`src/application.rs`); construction happens in `main` (the composition root).
- **Rationale**: Application state is an application-layer concern; main is the composition root per the framework-free DI pattern. The banner flows through `AppState` so the seam is exercised.
- **Alternatives considered**: `infrastructure::composition` submodule — viable and referenced by research, but adds a nesting level with no real infrastructure to contain yet; merged into `application` for now.

## 7. Migration of Existing Foundation Code

- **Decision**: `app_version()` remains in the crate root; `main` moves banner printing behind an `AppState` method. Zero new dependencies; only invisible-behavior changes to `main`.
- **Rationale**: FR-008 requires unchanged observable behavior; the composition seam must be exercised.
- **Alternatives considered**: Leaving `main` untouched — rejected (then FR-006 has no demonstrator).