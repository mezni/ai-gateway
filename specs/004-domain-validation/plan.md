# Implementation Plan: Domain Model Validation

**Branch**: `004-domain-validation` | **Date**: 2026-09-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-domain-validation/spec.md`

## Summary

Turn the gateway's minimum-shape chat handling into one documented, ordered, and
deterministic validation contract covering `model`, `messages`, `temperature`,
`max_tokens`, `stream`, and whole-request size. Generation controls become
optional typed fields on the provider-independent domain request, so a control is
either supplied with its exact value or recorded as unspecified and never
replaced by an invented default. A dedicated validation stage returns the single
opaque value that application logic may receive, a fixed 1 MB whole-body bound is
enforced ahead of media-type and structural handling with its own documented
response, and the existing flat two-field client error contract is preserved
verbatim with a single new row. No provider, credential, database, cache, endpoint,
or client-visible field set is introduced.

## Technical Context

**Language/Version**: Rust 1.98.1, Edition 2024

**Primary Dependencies**: Axum 0.8, Tokio 1.x, Serde 1.x, serde_json 1.x,
thiserror 2.x, and anyhow 1.x; `http-body-util` 0.1 for `LengthLimitError` when
distinguishing an oversized body from an unreadable one; Tower 0.5 with `util` for
router tests

**Storage**: N/A; all state is process-local and non-durable

**Testing**: `cargo test`; unit tests, in-process Axum router tests with
`ServiceExt::oneshot`, and the existing bound-port lifecycle tests on `127.0.0.1`

**Target Platform**: Local Linux server with Ctrl-C and SIGTERM support

**Project Type**: Single Rust library plus thin binary web service

**Performance Goals**: At least 95% of 100 canonical chat requests complete within
250 ms; a body of exactly 1 MB is accepted and one byte more is refused; a
refused oversized body does not degrade later request handling

**Constraints**: Whole-request-body limit fixed at 1 MB (1 048 576 bytes,
inclusive); the client error contract stays exactly `{code, message}` with no
`details` field; the seven existing error rows keep their status, code, and
message; the only new row is 413 `payload_too_large`; no provider, credential,
database, cache, persistence, authentication, authorization, rate limiting,
routing, token counting, or streaming implementation

**Scale/Scope**: One local gateway process, three endpoints, six validation
concerns, one generation-control pair, no durable data

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Initial gate: PASS (pre-research).** The active specification is complete, it
contains no unresolved `[NEEDS CLARIFICATION]` markers, and the proposed work
introduces only validation required by Phase 3 of `docs/plan.md`. No user
clarification is required.

**Post-design re-check: PASS (after Phase 0 and Phase 1).** The design below was
re-evaluated against every MUST-level principle after
[research.md](research.md), [data-model.md](data-model.md),
[contracts/validation-rules.md](contracts/validation-rules.md),
[contracts/http-api.md](contracts/http-api.md), and
[quickstart.md](quickstart.md) were produced. No violation is accepted or
deferred.

| Principle / Gate | Status | Design evidence |
|------------------|--------|-----------------|
| Specification-driven development | PASS | FR-001..FR-027 map to the rule catalog in `contracts/validation-rules.md`, the entities in `data-model.md`, the wire contract in `contracts/http-api.md`, and the scenarios in `quickstart.md`. `tasks.md` remains the Phase 2 output. |
| Rust-first engineering | PASS | Rust 1.98.1; `Option<f64>`/`Option<u32>` move type errors to compile time where possible; validation is pure and stateless; no `unsafe` is planned; illegal control values are unrepresentable in the validated request. |
| Incremental architecture | PASS | Adds only the validation Phase 3 declares. No provider adapter, model registry, routing, auth, rate limit, persistence, cache, or telemetry enters the design. Fixed 1 MB bound and per-field limits are explicitly deferred to the security and domain-validation follow-up phases. |
| Provider independence | PASS | The validated request gains only provider-agnostic `temperature` and `max_tokens`; no provider shape, provider selection, or provider concept is introduced; `MockChatCompletionService` remains the sole execution path. |
| Explicit boundaries | PASS | Domain owns `ChatRequest` generation controls and range constants and imports no framework; API owns DTOs, media type, size enforcement, and error mapping; application owns the ordered rule evaluation and domain mapping; config is unchanged. Business validation stays out of the HTTP handler. |
| Reliability and failure isolation | PASS | Validation is pure and cannot fail partially; exactly one response is produced per request by the documented precedence order; an oversized or abandoned body is stopped without touching gateway state; readiness and graceful shutdown behavior are unchanged and re-verified. |
| Security and privacy | PASS | Bounded 1 MB whole-body limit is introduced here as a DoS precondition; failure responses never echo the rejected value, prompt content, parser output, or diagnostics; no credentials are read; the API remains explicitly local and unauthenticated. |
| Observability | PASS (phase scope) | Liveness and readiness keep reporting the gateway's honest ability to serve; the specification defers production telemetry, and no logging of request content is introduced. |
| Testability and correctness | PASS | Every rule in the catalog has a named automated check; boundary values, precedence combinations, repetition, concurrency isolation, and offline operation are covered; both success and failure paths are tested; no external service is required. |
| Performance and resource discipline | PASS | Async I/O only; a declared `Content-Length` over the limit is refused without reading the body; the accepted body is read at most twice within a 1 MB bound; no cloning of message content beyond the existing single validated value; no blocking work on runtime workers. |
| Documentation and governance | PASS | `docs/api.md`, `docs/testing.md`, `README.md`, `CHANGELOG.md`, and `HANDOFF.md` updates are planned alongside the shipped rule catalog, which is the user-facing source of truth for FR-027. |

**Complexity exceptions**: None.

### Interpretations Recorded

Two readings were available under the constitution and are recorded here so a
reviewer does not have to reconstruct them. Neither is a violation.

| Question | Reading applied | Why |
|----------|-----------------|-----|
| Does "Configuration MUST be externalized" require the 1 MB body limit to become an environment variable? | No. The limit stays a fixed constant. | The rule governs configuration — application, provider, security, routing, operational settings — not every literal. This phase introduces no new configurable setting, and FR-010 fixes the number while the specification defers configurability to the security phase. Principle III forbids introducing configuration surface before a specification requires it. |
| Does request-size enforcement belong in the API layer, given the constitution lists "Security" as its own boundary? | Yes, it stays in the API layer as a chat-route stage. | Bounded buffering before deserialization is transport admission, the same class of concern as the existing media-type and method handling already owned by the API layer. The *business* rules — which fields, which ranges, which responses — remain in the application and domain layers, so Principle V's "business logic must not be embedded in HTTP handlers" is respected: the handler contains no validation branch. |

### Success-Criteria Verification

| Criterion | Planned verification |
|-----------|----------------------|
| SC-001 | `contracts/validation-rules.md` plus `contracts/http-api.md` state every rule, range, boundary, and response; a reader predicts responses without reading source. |
| SC-002 | Each catalog rule ID maps to at least one named automated check; a coverage test asserts the ID list is fully exercised. |
| SC-003 | Repeat the canonical request 100 times with and without generation controls; assert identical status and body and no invented control value. |
| SC-004 | Assert 0.0/2.0 temperature and 1/4096 `max_tokens` accepted, 2.0001 and 4097 refused, and bodies of exactly 1 048 576 bytes accepted and 1 048 577 bytes refused. |
| SC-005 | For each documented failure condition, assert exact status/code/message and assert the body contains no rejected value, prompt marker, parser message, or internal detail. |
| SC-006 | Repeat each combined-violation request 20 times and assert one stable documented response per combination. |
| SC-007 | Assert the application service is only reachable through the validated value and that no unvalidated request type enters application logic. |
| SC-008 | Fire 50 concurrent valid and invalid requests and assert isolated correct results. |
| SC-009 | Assert liveness and readiness before, during, and after valid, invalid, and oversized requests; assert a normal request succeeds after a refusal. |
| SC-010 | Run `fmt --check`, `clippy --all-targets -- -D warnings`, `check --all-targets`, `test --all-targets`, and both builds; assert all three endpoints still serve. |

## Project Structure

### Documentation (this feature)

```text
specs/004-domain-validation/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── spec.md
├── checklists/
│   └── requirements.md
└── contracts/
    ├── http-api.md
    └── validation-rules.md
```

### Source Code (repository root)

```text
src/
├── api.rs
├── api/
│   ├── chat.rs
│   ├── dto.rs
│   ├── error.rs
│   ├── health.rs
│   ├── middleware.rs
│   └── server.rs
├── application.rs
├── application/
│   ├── chat.rs
│   └── lifecycle.rs
├── config.rs
├── config/
│   └── server.rs
├── domain.rs
├── domain/
│   ├── catalog.rs
│   └── chat.rs
├── infrastructure.rs
├── lib.rs
└── main.rs
tests/
├── http_api.rs
└── server_lifecycle.rs
```

This phase adds no new module files. It extends existing ones:

| File | Change |
|------|--------|
| `src/domain/chat.rs` | Add `temperature: Option<f64>` and `max_tokens: Option<u32>` to `ChatRequest`, range constants, and a `with_controls` constructor beside the existing `new`. |
| `src/api/dto.rs` | Add optional `temperature`/`max_tokens` to the request DTO and replace the derived deserializer with one that rejects duplicate and mistyped controls. |
| `src/api/error.rs` | Add the `PayloadTooLarge` variant mapped to 413 `payload_too_large`; leave every existing row unchanged. |
| `src/api/middleware.rs` | Add the whole-request-size stage ahead of the JSON extractor. |
| `src/api/chat.rs` | Map the extended DTO into the extended command; keep the streaming check last. |
| `src/application/chat.rs` | Own the ordered rule evaluation, the control range checks, and the domain mapping. |
| `src/api/server.rs` | Compose the size stage outside the existing admission middleware on the chat route. |
| `tests/http_api.rs` | Add rule-by-rule, boundary, precedence, repetition, concurrency, and offline coverage. |
| `Cargo.toml` / `Cargo.lock` | Declare `http-body-util` for `LengthLimitError`. |

Documentation updated with the shipped behavior:

```text
README.md
CHANGELOG.md
HANDOFF.md
docs/api.md
docs/testing.md
docs/plan.md
specs/003-http-gateway-core/contracts/http-api.md
```

**Structure Decision**: Keep the existing single-crate, file-stem module
convention established in Phase 1 and unchanged since. Validation is a
responsibility of the application layer, so it is added to
`src/application/chat.rs` rather than a new module: it is one ordered rule set
over one command type, and a separate `validation.rs` would split a single
cohesive contract across two files. The wire-format decisions (media type, body
size, duplicate keys, control typing) stay in the API layer, where the transport
is visible. The domain gains the two provider-agnostic control fields and the
range constants because it owns the invariants, and it keeps no framework
dependency.

## Complexity Tracking

No constitution violations or undocumented complexity exceptions.
