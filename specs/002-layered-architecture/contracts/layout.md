# Module Boundary Contract

**Feature**: [Layered Architecture](../spec.md)
**Date**: 2026-09-24

## Purpose

Defines the architectural contract for the gateway's layered structure. This
contract governs where code lives and which layers may depend on which. It is
the enforcement mechanism for the project constitution's Explicit Boundaries
principle.

## Layers

| Layer          | Source root          | Owns                                                                            | May depend on                         |
|----------------|----------------------|---------------------------------------------------------------------------------|---------------------------------------|
| Domain         | `src/domain`         | Chat concepts, catalog concepts (model/provider)                                | standard library only                 |
| Application    | `src/application`    | Orchestration, `AppState` composition root, lifecycle                           | domain, config, `std`, `tokio` sync   |
| API            | `src/api`            | HTTP transport, request/response DTOs, routing, middleware, error normalization | application, config, transport crates |
| Infrastructure | `src/infrastructure` | Adapters, persistence, external integrations (placeholder)                      | domain, config                        |
| Config         | `src/config`         | Runtime configuration types and loading                                         | standard library only                 |

## Dependency Rules

1. Dependencies point **inward only**: `api → application → domain` and
   `infrastructure → domain`.
2. The `domain` layer MUST NOT import `application`, `api`, `infrastructure`,
   or `config`.
3. `api` MAY import `application`, `config`, and third-party transport crates
   (`axum`, `serde`, `tokio`, `http`). It MUST NOT import `domain` types: the
   API layer talks only to the application layer (`application::chat` and
   `application::AppState`), and translation between transport and domain
   happens in `application`.
4. `application` MAY import `domain` and `config`, plus the standard library
   and `tokio` synchronization primitives (`tokio` is an async runtime, not a
   transport crate, so it is permitted here). It MUST NOT import `api` or
   `infrastructure`, and MUST NOT return or import an API error type: the API
   layer maps application errors such as `InvalidChatRequest` to its own
   transport error at the boundary.
5. `config` depends only on the standard library (`std::env`,
   `std::net::IpAddr`) and `thiserror`; it MUST NOT depend on any other
   project layer.
6. `domain` stays free of transport, serialization, and async-runtime crates:
   no `axum`, no `tokio`, no `serde`, no HTTP types. Its types are plain Rust
   structs and enums with hand-written constructors.
7. No layer MAY depend on a more-external layer than itself.
8. Provider model stays in the domain as a concept; provider *implementations*
   belong in `infrastructure`.
9. `infrastructure.rs` remains an empty placeholder module for future adapters
   and providers; it has no dependencies yet.

## Crate Accessibility

- Public API surface is exposed from the lib crate root (`src/lib.rs`).
- Integration tests use `use ai_gateway::domain::chat::ChatRequest;` etc.
- `main.rs` is the composition root: it constructs `AppState` and delegates;
  it MUST NOT contain domain logic.

## Violation Handling

Any import that crosses the dependency rules is a build/design violation and
MUST be rejected in review. The module structure makes accidental violations
visible (a module tree inspection).

## Maintenance

New modules MUST be added to this contract before implementation of the phase
that introduces them.