# Module Boundary Contract

**Feature**: [Layered Architecture](../spec.md)
**Date**: 2026-09-24

## Purpose

Defines the architectural contract for the gateway's layered structure. This
contract governs where code lives and which layers may depend on which. It is
the enforcement mechanism for the project constitution's Explicit Boundaries
principle.

## Layers

| Layer            | Source root      | Owns                                                   | May depend on        |
|------------------|------------------|--------------------------------------------------------|----------------------|
| Domain           | `src/domain`     | Chat concepts, catalog concepts (model/provider)       | nothing              |
| Application      | `src/application`| Orchestration, `AppState` composition root             | domain, config       |
| API              | `src/api`        | HTTP/transport exposure (future)                       | application, config  |
| Infrastructure   | `src/infrastructure` | Adapters, persistence, external integrations (future)| domain, config       |
| Config           | `src/config`     | Configuration types and loading (future)               | domain               |

## Dependency Rules

1. Dependencies point **inward only**: `api → application → domain` and
   `infrastructure → domain`.
2. The `domain` layer MUST NOT import `application`, `api`, `infrastructure`,
   or `config`.
3. `application` MUST NOT import `api` or `infrastructure`.
4. No layer MAY depend on a more-external layer than itself.
5. Provider model stays in the domain as a concept; provider *implementations*
   belong in `infrastructure`.
6. During this phase, `api.rs`, `infrastructure.rs`, and `config.rs` are
   empty placeholder modules; their rules take effect when implemented.

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