# Research: Domain Model Validation

**Feature**: [spec.md](spec.md) — Phase 3 of the AI Gateway roadmap

**Date**: 2026-09-30

**Status**: All technical unknowns resolved

The active Phase 3 specification is authoritative. Where the broader target
documents describe provider routing, per-model limits, configurable size limits,
`details` error fields, or token counting, those belong to later phases and are
not implemented here. `docs/plan.md` §8 is the roadmap entry this phase
realizes; `docs/api.md` is the target API this phase implements a documented
subset of.

## 1. Where Generation Controls Live

- **Decision**: Add `temperature: Option<f64>` and `max_tokens: Option<u32>` as
  direct fields on `domain::ChatRequest`, exactly as `docs/api.md` §34 sketches.
  Keep the existing `ChatRequest::new(model, messages)` and add
  `ChatRequest::with_controls(model, messages, temperature, max_tokens)`. `new`
  sets both controls to unspecified, so every existing domain and application
  assertion that constructs a request without controls keeps compiling and
  passing unchanged. `ValidatedChat` exposes the controls through accessors.
- **Rationale**: `Option` is the honest representation required by FR-006 and
  FR-007: absent stays absent, and a supplied value is carried exactly. Making
  the controls a property of the domain request rather than the transport DTO
  satisfies FR-014 — the only value reaching application logic is the validated
  request — and prepares Phase 4, whose `LlmProvider::chat(&ChatRequest)` will
  need them without reaching back into the API. Keeping `new` intact avoids
  churning the Phase 2 test suite for a purely additive change.
- **Alternatives considered**: A nested `GenerationControls` struct was rejected
  because `docs/api.md` defines the fields flat, and a third type between the
  request and its fields buys nothing while adding a name to learn. A separate
  `ValidatedRequest` type distinct from `ChatRequest` was rejected because it
  duplicates the model and message list and creates two request concepts before
  a provider exists. `f32` as literally written in `docs/api.md` was rejected
  because that section is explicitly conceptual and because `f64` is JSON's
  native number type, so `f32` would add a lossy conversion step with no
  benefit at a 0.0–2.0 range. Storing controls only on the application
  `ValidatedChat` was rejected because the next phase needs them on the value
  handed to a provider.
- **References**: `docs/api.md` §34, `src/domain/chat.rs`,
  `specs/002-layered-architecture/contracts/layout.md`.

## 2. Strict Control Typing and Refusing Non-Numbers

- **Decision**: Deserialization of `temperature` accepts only a JSON number and
  converts it to `f64`; `max_tokens` accepts only a JSON integer in `u32` range.
  Every other JSON type — string, boolean, `null`, object, array — is a
  deserialization failure and therefore 400 `invalid_request`. This is why the
  DTO's control fields are read through a hand-written deserializer rather than
  `Option<f64>`: `Option<f64>` deserializes `null` to `None`, which would make a
  client-supplied `null` indistinguishable from an omitted control and would
  silently violate FR-008. The same hand-written deserializer implements the
  duplicate-key rule, so the two requirements are solved by one component.
- **Rationale**: FR-008 enumerates `null` explicitly, so the "absent" and "null"
  cases must be distinguishable at the type level. Making the field's
  deserializer reject `null` outright is the only way to keep that distinction
  without inspecting the raw JSON a second time. A fractional or exponent-form
  `max_tokens` such as `100.5` or `1e3` is refused: a token count is an integer,
  and the specification's boundary values (1, 4096, 4097) are integers. Every
  rejection path converges on the same documented row, so no new client-visible
  distinction is created.
- **Alternatives considered**: Accepting `100.0` for `max_tokens` by first
  reading a float and checking integrality was rejected because it makes the
  accepted language of the field depend on a rounding decision and would accept
  a value a client could not have produced by integer arithmetic. A permissive
  `serde_json::Value` staging step followed by manual checks was rejected
  because it duplicates the type system in hand-written match arms and loses
  FR-008's compile-time-adjacent safety. Reporting a distinct code such as
  `invalid_temperature` was rejected by FR-017, which forbids new per-field
  detail in this phase.
- **References**: FR-004, FR-005, FR-008, `src/api/dto.rs`,
  `specs/003-http-gateway-core/research.md` §4.

## 3. Rejecting a Repeated Control

- **Decision**: Deserialize the chat request with a manual `Deserialize`
  implementation that visits the top-level JSON object through `MapAccess` and
  keeps a set of the keys it has already accepted. A second occurrence of
  `temperature` or `max_tokens` returns a deserialization error, which the
  existing `JsonRejection` mapping already turns into 400 `invalid_request`.
  Unknown keys are skipped, so FR-020 still holds, and the duplicate check is
  scoped to the two controls because FR-009 speaks about "the same control".
- **Rationale**: Serde's derived struct deserializer and `serde_json::Value` both
  collapse duplicate keys silently, keeping the last occurrence, so neither can
  satisfy FR-009. Detecting the repetition needs visibility across the whole
  object, which is exactly what a manual `MapAccess` loop provides. The seen-key
  set is created inside the visitor, so it is per-deserialization: concurrent
  requests cannot influence each other, which FR-022 and FR-015 require.
  Keeping the check in the same deserializer as the strict typing avoids two
  passes over the body and two error surfaces.
- **Alternatives considered**: A thread-local or task-local "seen keys" set was
  rejected: it is shared mutable state on the hot path, it leaks across requests
  if a task is reused, and it directly contradicts the isolation requirement.
  Rejecting any duplicate key rather than only control duplicates was rejected
  as stricter than FR-009 and as a gratuitous compatibility break for clients
  that repeat an ignored field. Scanning raw bytes for repeated key names was
  rejected because it matches keys inside string values and nested objects and
  would reject valid documents. A separate pre-parse pass was rejected as
  redundant once the manual deserializer exists.
- **References**: FR-009, FR-015, FR-020, FR-022,
  specification edge case "The same control is supplied more than once".

## 4. One Ordered Rule Set

- **Decision**: Enumerate the rules as a numbered catalog in
  `contracts/validation-rules.md`, each with a stable identifier, the condition
  it governs, and the single response it produces. The application evaluates the
  catalog in the precedence order the specification fixes — required-field
  validity, then control-range validity — returning on the first violation. The
  streaming check stays in the API layer, after the application accepts the
  command, which is what makes it the last stage in FR-016. Document the
  catalog as the single source of truth and reference it from the wire contract
  and the quickstart instead of restating it.
- **Rationale**: FR-015 and FR-016 make determinism and order part of the public
  contract, so the order has to be written down once and be the order the code
  uses. A catalog with identifiers is also what SC-002 can be measured against:
  every identifier must map to at least one automated check. Splitting the rules
  across `docs/api.md`, the contract, and the code is how they drift, so the
  other documents link to the catalog.
- **Alternatives considered**: Reporting every violated rule at once was rejected
  because FR-015 and FR-017 require exactly one response and forbid a per-field
  list. Returning the violated field name to help clients was rejected by FR-017
  and by the specification's explicit decision to keep the error contract stable
  for existing clients. A table-driven rule registry with closures was rejected
  as unnecessary indirection for six concerns and harder to read than an ordered
  `if`-chain that literally mirrors the documented order. Validating in the API
  handler was rejected by Principle V.
- **References**: FR-001, FR-015, FR-016, FR-017, `docs/plan.md` §8.

## 5. The 1 MB Whole-Body Limit and Its Position in Precedence

- **Decision**: Enforce the limit in a chat-route layer that runs after
  admission and before the JSON extractor. The stage rejects a declared
  `Content-Length` above 1 048 576 bytes without reading the body at all, then
  otherwise reads the body through `axum::body::to_bytes(body, limit)`, which
  fails only when the body *exceeds* the limit, so exactly 1 MB is accepted. On
  success the request is rebuilt around the in-memory bytes with an explicit
  `DefaultBodyLimit::max(1 MB)` extension so the endpoint's real limit is
  declared in code rather than inherited from the framework default. Because
  this stage sits outside the extractor, the size decision precedes the media
  type and structural checks, which is what FR-016 requires.
- **Rationale**: Axum's `Json` extractor checks the content type before it
  buffers the body, so a limit enforced only through `DefaultBodyLimit` would
  report 415 for an oversized request with a wrong media type, contradicting
  FR-016 and the specification's edge case that an oversized body reports the
  size limit. `to_bytes` uses a limited reader whose error source is
  `http_body_util::LengthLimitError`, which lets an oversized body be reported
  as 413 while a body that merely failed to read keeps the Phase 2 400
  `invalid_request` treatment. The declared-length fast path means an
  oversized request is refused without buffering, which is the DoS behavior
  FR-012 asks for. Reading the body once in the layer and once more in the
  extractor costs at most one extra copy of a bounded 1 MB buffer and keeps the
  entire Phase 2 media-type and error mapping untouched, which is worth more
  than the copy.
- **Alternatives considered**: `DefaultBodyLimit::max` alone was rejected for
  the ordering reason above. `tower_http::limit::RequestBodyLimitLayer` was
  rejected because it aborts the response rather than returning the documented
  error row, so the client contract would depend on transport-level framing.
  Reading and parsing the body in the layer and calling `serde_json` there was
  rejected because it would hand-roll the media type check and duplicate axum's
  accepted-suffix logic. Counting bytes as frames arrive with a custom
  `http_body::Body` wrapper was rejected as more machinery than `to_bytes`
  requires for a fixed bound. A configurable limit was rejected because the
  specification fixes 1 MB for this phase and defers configurability to the
  security phase. A per-endpoint limit of a different value was rejected
  because FR-010 fixes the number.
- **References**: FR-010, FR-011, FR-012, FR-016,
  `specs/003-http-gateway-core/research.md` §4,
  `specs/003-http-gateway-core/contracts/http-api.md` §5.

## 6. The One New Error Row

- **Decision**: Add a `PayloadTooLarge` variant to `ApiError` mapped to HTTP 413
  with code `payload_too_large` and the exact fixed message `The request
  payload is too large.`, rendered through the same `ApiErrorDto` as every other
  row so the object still contains exactly `code` and `message`. The seven
  existing rows keep their status, code, and message unchanged.
- **Rationale**: FR-019 requires the oversize response to be distinguishable
  from other client errors, and FR-010 forbids reusing the generic
  `invalid_request` row. Adding one variant to the existing enum is the
  smallest change that satisfies both, and it automatically inherits the
  existing per-row tests that assert exactly two fields and no diagnostics.
- **Alternatives considered**: Mapping oversize to 400 `invalid_request` was
  rejected by FR-010 and FR-019. Using 413 with `invalid_request` was rejected
  because FR-019 names a distinct code. Introducing a `details` payload such as
  the observed length was rejected by FR-017 and would leak request shape.
  Returning a bare 413 with no body was rejected because the contract requires
  the same two-field object for every request-processing error.
- **References**: FR-010, FR-017, FR-019, FR-023,
  `src/api/error.rs`.

## 7. Keeping the Validated Value Opaque

- **Decision**: `MockChatCompletionService::validate` keeps returning the opaque
  `ValidatedChat`, extended to carry the validated controls and to expose
  `model()` and `controls()` accessors. `complete` continues to consume a
  `ValidatedChat` by value, so application logic cannot be handed an unvalidated
  command. The API handler keeps receiving a DTO, mapping it to
  `CompleteChatCommand`, and never inspecting domain types.
- **Rationale**: FR-014 requires a single validated value to be the only request
  form reaching application logic. Consuming the validated value by value makes
  that a property of the signature rather than a convention, and it already
  works this way. Extending the accessors rather than exposing the inner
  `ChatRequest` preserves the Phase 2 boundary decision that the API must not
  import domain types.
- **Alternatives considered**: Passing the raw command plus a `Result` of
  validation into `complete` was rejected because it lets an unvalidated request
  reach application logic. Returning the domain `ChatRequest` to the API was
  rejected as the boundary reversal already considered and rejected in Phase 2.
  A newtype per control was rejected as ceremony for two `Option` fields.
- **References**: FR-014, FR-022, `src/application/chat.rs`,
  `specs/002-layered-architecture/contracts/layout.md`.

## 8. Test Strategy

- **Decision**: Extend the existing layered suite rather than introducing a new
  harness. Unit tests in the API and application modules cover deserialization
  strictness, duplicate keys, each range boundary, and the ordered rule set
  directly. Router tests with `ServiceExt::oneshot` cover every rule's response
  row, the precedence combinations, body-size boundaries at exactly 1 MB and one
  byte more, no-echo assertions using sentinel markers, 100 repetitions, and 50
  concurrent mixed requests. The existing bound-port lifecycle tests continue to
  prove liveness, readiness, admission, and shutdown.
- **Rationale**: These are pure decisions about request values and HTTP
  responses, so in-process router tests are both sufficient and free of network
  flakiness. Boundary values are cheap to generate in memory, which makes exact
  1 MB and 1 MB + 1 byte tests straightforward. Sentinels embedded in rejected
  values are the direct way to prove FR-018 rather than asserting only the
  message text.
- **Alternatives considered**: Property-based testing would be a good fit for
  the range rules but adds a dependency the project has not adopted, and the
  boundaries are finite and enumerable. A full external HTTP client was rejected
  because nothing about these rules requires a real socket, and the real-socket
  path is already covered by the lifecycle tests. Snapshot testing of error
  bodies was rejected because the existing suite already asserts exact JSON
  objects field by field.
- **References**: SC-002, SC-003, SC-004, SC-005, SC-006, SC-008, SC-009,
  `specs/003-http-gateway-core/research.md` §9.

## 9. Scope Boundaries

- **Decision**: Do not add provider clients, model registry or routing,
  authentication, authorization, rate limiting, token counting, per-field content
  limits, a maximum message count, per-message size limits, a configurable size
  limit, security headers, CORS, request IDs, structured logging, metrics,
  tracing, persistence, caching, or deployment packaging. Do not change the
  success response envelope, the routing table, or any of the seven existing
  error rows. Do not present a generation control as having influenced the mock
  response.
- **Rationale**: Each of these is assigned to a later roadmap phase, and
  introducing any of them would violate incremental delivery. The specification
  is explicit that per-field and per-message limits are deferred to a
  domain-validation follow-up and that the whole-body limit is the only size
  rule here. Because no model is connected, the mock must keep returning a fixed
  completion; a control that appeared to change the output would be dishonest.
- **Alternatives considered**: Implementing the full `docs/api.md` request and
  error envelope was rejected because the active specification intentionally
  defines a narrower contract. Adding per-message length limits alongside the
  body limit was rejected as unrequested scope. Echoing the accepted controls in
  the success response was rejected because no new client-visible field set is
  introduced in this phase and the mock cannot honor them.
- **References**: specification Assumptions, FR-020, FR-025, `docs/plan.md` §8.
