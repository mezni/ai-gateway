# Data Model: Layered Architecture

**Feature**: [spec.md](spec.md)
**Date**: 2026-09-24

This phase defines the domain entities (provider-independent) plus the module
boundary entities of the architecture skeleton. Attribute depth is "essential
fields only" per the clarify session (Option A); validation and provider
mapping are deferred to later phases.

## Entities

### ChatRequest

A client's request for a chat completion.

| Attribute | Type                | Rules                                    |
|-----------|---------------------|------------------------------------------|
| model     | String              | Model reference the request targets      |
| messages  | Vec&lt;Message&gt;    | At least the messages to send; owned list |

### Message

A single exchange within a chat.

| Attribute | Type            | Rules                                  |
|-----------|-----------------|----------------------------------------|
| role      | MessageRole     | MUST be one of the defined roles       |
| content   | String          | Text payload of the message            |

### MessageRole

The role of a message.

| Variants | Meaning                       |
|----------|-------------------------------|
| System   | System/instructions messages   |
| User     | End-user messages              |
| Assistant | Assistant/model replies       |

### ChatResponse

The gateway's normalized response to a chat request.

| Attribute | Type             | Rules                                         |
|-----------|------------------|-----------------------------------------------|
| content   | String           | Response text                                 |
| usage     | Option&lt;Usage&gt; | Token accounting for the request, when known  |

`Some(Usage)` means measured token accounting is available. `None` means token
accounting is unavailable. A `without_usage(content)` constructor builds the
`None` case.

### Usage

Token accounting for a request.

| Attribute           | Type | Rules                              |
|---------------------|------|------------------------------------|
| prompt_tokens       | u64  | Input tokens, non-negative count   |
| completion_tokens   | u64  | Output tokens, non-negative count  |
| total_tokens        | u64  | Total = prompt + completion (maintained by constructor) |

Validation: `total_tokens` MUST equal `prompt_tokens + completion_tokens`; a
constructor enforces this.

### Model

A model identifier as understood by the gateway.

| Attribute | Type   | Rules                    |
|-----------|--------|--------------------------|
| id        | String | Stable identifier        |

### Provider

An LLM service the gateway can route to, without vendor coupling.

| Attribute | Type   | Rules              |
|-----------|--------|--------------------|
| id        | String | Stable identifier  |
| name      | String | Human-readable name|

## Relationships

```text
ChatRequest ──carries──► Vec<Message> ──role──► MessageRole
ChatResponse ──carries──► Option<Usage> (optional)
Model, Provider         (catalog concepts; referenced by name/id by later routing phases)
```

`Model` and `Provider` are independent value concepts in `catalog`; later
routing phases reference them by id. No state transitions apply (immutable
value types).

## Phase 2 Note (HTTP Gateway Core)

The HTTP Gateway Core phase returns a deterministic local mock completion built
with `without_usage`, so no token counts are ever invented or reported. The
HTTP success envelope deliberately omits any `usage` or timestamp field.
Measured usage arrives with the provider/usage-reporting phase.

## Module Boundaries (architecture entities)

| Layer          | Owns                                                                    | May depend on                         |
|----------------|-------------------------------------------------------------------------|---------------------------------------|
| domain         | ChatRequest, Message, MessageRole, ChatResponse, Usage, Model, Provider | standard library only                 |
| application    | AppState composition root, chat orchestration, lifecycle                | domain, config, `std`, `tokio` sync   |
| api            | HTTP transport layer, DTOs, routing, middleware, error normalization    | application, config, transport crates |
| infrastructure | adapters, persistence, providers (placeholder)                          | domain, config                        |
| config         | runtime configuration types                                             | standard library only                 |

Rules: dependencies point INWARD only; no layer imports a more-external layer.
`infrastructure` is still an empty placeholder; `api`, `application`, and
`config` are implemented.

## Validation Rules (mapped from spec requirements)

- FR-002/SC-001: all planned layers exist as distinct modules and compile.
- FR-003/SC-002: all seven domain types exist and are exercised by tests.
- FR-004: each type carries the essential fields above.
- FR-005: no provider-specific concept appears in the domain model.
- FR-006: `AppState` provides the composition seam.
- FR-007/FR-008/SC-003/SC-004: application runs with no provider, banner unchanged, regression green.
- FR-010/SC-005: quality gates green.