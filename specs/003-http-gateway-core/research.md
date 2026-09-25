# Research: HTTP Gateway Core

**Feature**: [spec.md](spec.md) — Phase 2 of the AI Gateway roadmap

**Date**: 2026-09-25

**Status**: All technical unknowns resolved

The active Phase 2 specification is authoritative where the broader target
documents describe authentication, provider routing, streaming, usage, request
IDs, or dependency-aware readiness that are not implemented in this phase.

## 1. Minimal Async HTTP Stack

- **Decision**: Use Axum 0.8 on Tokio 1.x. Declare `axum`, `tokio`, `serde`,
  `serde_json`, `thiserror`, and `anyhow`; add Tower 0.5 with `util` as a
  development dependency for `ServiceExt::oneshot`.
- **Rationale**: Axum is already the documented HTTP framework and provides
  extractors, routing, JSON, fallbacks, state sharing, middleware, and graceful
  server shutdown without a custom HTTP stack. Tokio supplies the runtime,
  listener, timers, synchronization, and process signals. Broad compatible
  version ranges are declared and `Cargo.lock` pins the resolved releases.
- **Alternatives considered**: A raw Hyper service was rejected because the
  phase explicitly teaches Axum. Actix Web was rejected because it is not the
  selected stack. Reqwest is unnecessary because no provider or remote test
  service is used. Tower HTTP, dotenv, a configuration framework, tracing, and
  OpenTelemetry are deferred until a requirement needs them.
- **References**: Axum and Tokio crate documentation; project technology stack
  in `README.md`.

## 2. API DTO Boundary and Unavailable Usage

- **Decision**: Keep HTTP serialization and JSON shapes in `src/api/dto.rs`.
  Map a DTO to an application-owned `CompleteChatCommand`; the application
  validates it and maps it to the existing domain request. Return a small
  application-owned completion projection for the API rather than exposing a
  domain type from the handler. Change `domain::ChatResponse.usage` from `Usage`
  to `Option<Usage>`, preserve the existing constructor by wrapping supplied
  usage in `Some`, and add a no-usage constructor for the mock.
- **Rationale**: Serde and Axum do not leak into the domain, and the API imports
  only inward-facing application/config modules as required by the authoritative
  layer contract. Unavailable usage is represented honestly rather than
  fabricated as zero-valued usage. The normalized provider design already uses
  `Option<Usage>`. This is a narrow correction to the Phase 1 placeholder, not
  the comprehensive validation work assigned to Phase 3.
- **Alternatives considered**: Deriving Serde on domain types was rejected
  because it couples the core model to transport. Mapping the DTO directly to
  domain types inside the API was rejected because it bypasses the permitted
  API-to-application boundary. Returning `Usage::new(0, 0)` and hiding it in the
  response DTO was rejected because it still records invented measurements.
  Exposing a wire DTO from the application was rejected; the small application
  result is transport-neutral and contains only the Phase 2 output needed by the
  API.

## 3. Deterministic Success Contract

- **Decision**: Return the target non-streaming response shape with fixed
  `id: "chat_mock"`, `object: "chat.completion"`, the requested `model`, one
  choice at index 0, assistant role, fixed content, and `finish_reason: "stop"`.
  Omit `usage` and timestamps.
- **Rationale**: This follows `docs/api.md` where compatible while meeting the
  active specification's stronger determinism and no-usage requirements.
- **Alternatives considered**: Per-request identifiers were rejected because
  they would make identical mock responses differ. A response containing only
  model and choices was rejected because the established target contract already
  defines an OpenAI-compatible envelope. A `created` timestamp was rejected
  because it is not required and would break full-response determinism.

## 4. Minimal Request Validation and Error Normalization

- **Decision**: Parse only `model`, `messages`, and `stream` explicitly and
  default `stream` to false. The API maps structural data to an application
  command; the application requires a non-empty model, at least one message, a
  supported role, and non-empty string content, then returns an opaque validated
  command backed by the domain request. The API checks `stream: true` only after
  that validation succeeds. Ignore other JSON fields until later phases.
  Normalize every `JsonRejection` to the exact flat error contract, with 400 for
  malformed or structural data and 415 for missing/unsupported media types.
- **Rationale**: Serde is necessary at the HTTP boundary, but business validation
  remains in the application layer and comprehensive domain validation, numeric
  ranges, request-size policy, and token controls are explicitly deferred to
  Phase 3.
- **Alternatives considered**: Denying unknown fields was rejected because the
  target API already defines deferred options. Trimming strings was rejected
  because the specification requires non-empty values, not non-blank values.
  Surfacing Serde's default 422 response was rejected because the public
  contract requires 400. Separate public error codes for syntax and data errors
  were rejected because the phase defines one `invalid_request` row. Checking
  streaming before application validation was rejected so malformed requests
  containing `stream: true` still receive `invalid_request`.

## 5. Runtime Configuration

- **Decision**: Add `AI_GATEWAY_HOST` and `AI_GATEWAY_PORT`, defaulting to
  `127.0.0.1` and `3000`. Parse the host as an IPv4/IPv6 literal and the port as
  a non-zero `u16`; invalid or empty overrides fail startup. Do not auto-load a
  `.env` file. Keep the 10-second shutdown allowance fixed for this phase.
- **Rationale**: The names follow the authoritative environment contract, the
  configuration is sufficient for a local TCP listener, and parsing only typed
  literals avoids DNS and hidden configuration behavior.
- **Alternatives considered**: Generic `HOST`/`PORT` names were rejected
  because gateway variables must use the `AI_GATEWAY_` namespace. A hostname
  resolver was rejected because the milestone requires a local, deterministic
  listener and clear invalid-address failures. A shutdown-timeout environment
  variable was rejected because it is not required and could weaken the
  specification's 10-second bound.

## 6. Lifecycle and Admission Tracking

- **Decision**: Add an application lifecycle controller with
  `Initializing`, `Ready`, `ShuttingDown`, and `Stopped` phases, plus an active
  chat-request count. A chat-route middleware admits only `Ready` requests,
  returns the 503 `not_ready` contract otherwise, and holds an RAII guard until
  request completion or cancellation.
- **Rationale**: Liveness and readiness remain separate, readiness never
  reports a false dependency claim, and admission/shutdown ordering is testable.
  Atomic phase storage plus a Tokio watch channel for the active count avoids
  holding a lock across asynchronous work.
- **Alternatives considered**: Independent `ready` and `shutting_down` booleans
  can admit a request after shutdown begins. A request-count check in the
  handler is too late to cover extraction/cancellation. Sleeping for a fixed
  shutdown interval ignores in-flight completion. A provider trait is deferred
  to Phase 4 because no interchangeable provider exists yet.

## 7. Two-Stage Graceful Shutdown

- **Decision**: Production passes a Ctrl-C/SIGTERM future to the server runner.
  On that signal, atomically transition to `ShuttingDown`, keep the listener
  active while waiting for admitted chat requests to reach zero or the absolute
  10-second deadline, then trigger Axum graceful shutdown. Bound the complete
  operation from the original signal; if graceful connection close outlives the
  deadline, drop the server future and return a clear startup/shutdown error.
  Tests inject a oneshot signal and hold a real admitted HTTP request open with
  a deliberately partial JSON body; closing the client stream releases the
  guard, while a separate short-deadline case verifies forced termination.
- **Rationale**: Calling `with_graceful_shutdown` immediately on the OS signal
  stops listener acceptance and can make `/health` and `/ready` unreachable,
  violating FR-016. The two-stage coordinator keeps operational probes available
  while rejecting new chat work and still enforces a hard phase bound.
- **Alternatives considered**: Immediate Axum graceful shutdown was rejected by
  the specification. Always sleeping for 10 seconds was rejected because it
  delays uncontended shutdown. Calling `process::exit` was rejected because
  dropping the bounded server future and returning an error is safer and
  testable. A background service or external supervisor is unnecessary for the
  local milestone.

## 8. Error Contract and Exact Method Routing

- **Decision**: Register one exact-method dispatcher for each supported path,
  a global unknown-path fallback, and one central API error type for every
  request-processing failure. The dispatcher explicitly rejects every method
  except GET on liveness/readiness and POST on chat, including HEAD, and sets
  the corresponding `Allow` header. Operational health/readiness bodies remain
  `{ "status": ... }` and are not wrapped in the error shape.
- **Rationale**: The public contract is stable, safe, independently testable,
  and literal about the supported methods. Explicit dispatch is required because
  Axum's `get` convenience routing also handles HEAD, while this phase defines
  only the listed methods.
- **Alternatives considered**: Axum's default empty 404/405 bodies were rejected
  because the specification requires JSON errors. Treating HEAD as an implicit
  alias was rejected because it would silently add a method not listed in the
  Phase 2 contract. Handlers returning ad hoc tuples were rejected because they
  would duplicate status/code/message logic.

## 9. Test Strategy

- **Decision**: Use unit tests for config, lifecycle/admission, command/DTO
  mapping, and error conversion; `ServiceExt::oneshot` for fast endpoint and
  failure-path tests; and a real `TcpListener` on port 0 for startup, injected
  shutdown, liveness/readiness during drain, new-request rejection, and
  process/socket cleanup. A raw TCP client sends an admitted POST with a partial
  body to prove cancellation releases the guard and drain waits correctly.
- **Rationale**: This covers pure behavior without network flakiness and also
  proves the long-running listener and shutdown coordinator work end to end.
- **Alternatives considered**: Unit tests alone would not exercise binding or
  graceful shutdown. A full external HTTP client is unnecessary for in-process
  router tests. A real provider or fixed port would violate offline and
  parallel-test requirements.

## 10. Scope Boundaries

- **Decision**: Do not add authentication, authorization, provider clients,
  routing, model registry behavior, streaming, token counting, persistence,
  caching, request IDs, production metrics, tracing, or deployment packaging.
- **Rationale**: These capabilities are assigned to later roadmap phases and
  would violate incremental delivery if pulled into Phase 2.
- **Alternatives considered**: Implementing the target API's full request and
  error envelope was rejected because the active specification intentionally
  defines a smaller local contract.
