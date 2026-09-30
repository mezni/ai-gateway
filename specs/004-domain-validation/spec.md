# Feature Specification: Domain Model Validation

**Feature Branch**: `004-domain-validation`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: "read from docs/plan.md phase 3"

## Overview

This phase turns the gateway's currently minimal chat request handling into a
stable, fully validated internal request model. Today the gateway accepts a
model identifier and a message list, and rejects anything that violates that
minimum shape. The declared roadmap for this phase requires the gateway to
validate `model`, `messages`, `temperature`, `max_tokens`, `stream`, and request
size, and to deliver a validated chat request that enters the application layer.

This phase therefore completes the request vocabulary: it defines the tuning
fields a client may send, establishes one consistent and ordered set of validation
rules, and guarantees that the only chat requests reaching application logic are
ones that already satisfy every rule. Validation failures continue to be reported
through the existing, unchanged, client-safe error contract.

The milestone requires no provider, credential, external service, or new
endpoint. Real model access, model routing, streaming, authentication, and
observability remain later capabilities.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Submit a Request with Generation Controls (Priority: P1)

An application developer sends a chat completion request that includes optional
generation controls — a sampling temperature and a response-length limit —
alongside the required model and messages. The gateway accepts the request when
the controls fall within the supported range, and the values the developer
supplied are carried through to the application layer unchanged rather than
being silently discarded or replaced with a default.

**Why this priority**: Generation controls are the most commonly used optional
fields in the chat vocabulary, and a gateway that drops them would make its
contract dishonest. Making them work correctly is the smallest slice of the
phase that delivers visible client value on its own.

**Independent Test**: Send a request containing in-range temperature and
max_tokens values, verify it is accepted, and verify the internal validated
request carries exactly the submitted values. This delivers working generation
controls without any external service.

**Acceptance Scenarios**:

1. **Given** a request with a valid model, at least one valid message, and an
   in-range temperature, **When** the client submits it, **Then** the gateway
   accepts the request and the validated request carries the submitted
   temperature.
2. **Given** a request with a valid model, at least one valid message, and a
   positive max_tokens value, **When** the client submits it, **Then** the
   gateway accepts the request and the validated request carries the submitted
   max_tokens.
3. **Given** a request that omits temperature and max_tokens entirely, **When**
   the client submits it, **Then** the gateway accepts the request and the
   validated request records that neither control was specified, rather than
   inventing a client-supplied value.
4. **Given** a request that omits both optional controls, **When** it is
   submitted twice with identical content, **Then** both responses are identical
   and neither response claims a control value that the client did not send.

---

### User Story 2 - Reject Out-of-Range Generation Controls (Priority: P1)

An application developer sends a request whose temperature or max_tokens value
is outside the supported range, or is not a well-formed value of the expected
kind. The gateway refuses the request with the documented invalid-request
response, identifies that the request is not usable, and does not disclose the
submitted value, internal parser output, or implementation detail.

**Why this priority**: Accepting an out-of-range control would push a bad value
deeper into the gateway where it becomes harder to diagnose and easier to misuse.
Rejecting it at the boundary is the whole point of a validation phase, so it
carries the same priority as accepting valid controls.

**Independent Test**: Submit requests with a temperature below the supported
minimum, a temperature above the supported maximum, a max_tokens value of zero
or negative, and non-numeric values for either control; verify each returns the
documented invalid-request status, code, and message and exposes no submitted
value or diagnostic.

**Acceptance Scenarios**:

1. **Given** a temperature below the supported minimum, **When** the client
   submits the request, **Then** the gateway returns the documented
   invalid-request response.
2. **Given** a temperature above the supported maximum, **When** the client
   submits the request, **Then** the gateway returns the documented
   invalid-request response.
3. **Given** a max_tokens value of zero or less, **When** the client submits the
   request, **Then** the gateway returns the documented invalid-request
   response.
4. **Given** temperature or max_tokens supplied as a value that is not a number,
   **When** the client submits the request, **Then** the gateway returns the
   documented invalid-request response rather than a server error.
5. **Given** a request is rejected for a control value, **When** the client
   inspects the response, **Then** the response contains no echo of the rejected
   value, no parser message, and no internal detail.
6. **Given** a request violates both a required field and a control range, **When**
   the client submits it, **Then** the gateway returns exactly one documented
   invalid-request response and does not report a different failure.

---

### User Story 3 - Submit a Reasonable-Sized Request (Priority: P1)

An application developer sends a request whose size is within the supported
limit. The gateway accepts it, and a request that exceeds the limit is refused
with a distinct, documented response rather than being partially processed. The
limit is large enough for realistic multi-turn conversations and prompts.

**Why this priority**: An unbounded request body is a denial-of-service vector,
and every other rule in this phase is meaningless if an oversized body is
accepted first. A bounded request size is a precondition for trusting the rest
of validation.

**Independent Test**: Submit a request just under the documented size limit and
verify it is accepted; submit a request exceeding the limit and verify it
returns the documented oversize response without partial processing; verify the
gateway remains healthy and serving afterwards.

**Acceptance Scenarios**:

1. **Given** a request whose size is within the supported limit, **When** the
   client submits it, **Then** the gateway accepts it and responds normally.
2. **Given** a request whose size exceeds the supported limit, **When** the
   client submits it, **Then** the gateway refuses it with the documented
   oversize response and does not return a completion.
3. **Given** a request refused for exceeding the size limit, **When** the client
   inspects the response, **Then** the response identifies that the request was
   too large and contains no internal detail.
4. **Given** a request refused for exceeding the size limit, **When** the client
   subsequently sends a normal request, **Then** the gateway still serves it
   successfully.
5. **Given** a client sends an oversized body and disconnects before receiving a
   response, **When** the gateway finishes handling it, **Then** the gateway
   remains healthy and continues serving later requests.

---

### User Story 4 - Trust One Consistent Validation Contract (Priority: P2)

An application developer or gateway maintainer can rely on a single, documented,
deterministic set of validation rules that applies to every chat request. The
rules are stated in one place, ordered so that a request violating several rules
always produces the same documented response, and enforced without requiring a
provider, credential, or external service.

**Why this priority**: Consistency and determinism are what make a validation
contract safe to build clients against and safe to test. They matter, but a
single inconsistent rule can be fixed without blocking any client journey, so
this story is secondary to the user-visible control and limit behavior.

**Independent Test**: Enumerate every documented validation rule, submit a
request violating each one individually and several in combination, and verify
every case returns the same documented response with no external service.

**Acceptance Scenarios**:

1. **Given** a request that violates multiple rules, **When** the client
   submits it, **Then** the gateway returns exactly one documented response, and
   repeating the same request returns the same response.
2. **Given** any documented invalid request, **When** the client submits it,
   **Then** the response contains exactly the documented machine-readable code
   and fixed message, and nothing else.
3. **Given** a documented valid request, **When** it is validated repeatedly or
   concurrently, **Then** the result is identical and no request influences
   another request's result.
4. **Given** validation runs with no provider, credential, database, cache, or
   other external service available, **When** a client submits valid and invalid
   requests, **Then** validation produces the same results and the gateway
   reports its own ability to serve traffic honestly.

---

### User Story 5 - Reject Explicit Streaming Requests (Priority: P2)

An application developer who requests a streamed response is told clearly that
streaming is not yet available, rather than silently receiving a complete
non-streaming answer that they may misread as a stream.

**Why this priority**: Silently downgrading a streamed request is a correctness
and trust problem for any client, so it must be explicit. It is already
observable in the gateway today, and this phase formalizes it as part of the
validated model rather than leaving it as a transport-only special case.

**Independent Test**: Submit a structurally valid request that explicitly asks
for a streamed response and verify the documented unsupported-feature response;
verify a request that omits the control is unaffected and still succeeds.

**Acceptance Scenarios**:

1. **Given** an otherwise valid request that explicitly requests a streamed
   response, **When** the client submits it, **Then** the gateway returns the
   documented unsupported-feature status, code, and message.
2. **Given** a request that is invalid on its own merits and also requests
   streaming, **When** the client submits it, **Then** the gateway returns the
   invalid-request response, not the unsupported-feature response.
3. **Given** a request that omits the streaming control, **When** the client
   submits it, **Then** the gateway processes it as a non-streaming request and
   the validated request records that no stream was requested.
4. **Given** a client inspects a rejected streaming request, **When** the client
   reads the response, **Then** the response does not contain a partial or
   complete completion body and contains no internal detail.

---

### Edge Cases

- A generation control is supplied with a value of the right kind but the wrong
  magnitude: refused with the documented invalid-request response.
- A generation control is supplied as a non-numeric value, including a numeric
  string, a boolean, null, an object, or an array: refused with the documented
  invalid-request response and never a server error.
- The same control is supplied more than once in a request: the request is
  refused rather than one occurrence being silently chosen.
- A request omits the generation controls entirely: accepted, and the validated
  request records them as unspecified rather than applying a client-visible
  default.
- A request exactly at the size limit: accepted; the boundary is inclusive.
- A request exceeds the size limit and the client never finishes sending the
  body: the gateway stops working on it, stays healthy, and serves later
  requests.
- A request is valid on its own but carries unknown extra fields: the unknown
  fields are ignored for this phase and do not cause a failure.
- A request is oversized and also invalid on its own merits: the size limit is
  reported, because the request cannot be validated without reading it.
- A request violates the model, message, control, and size rules at once: the
  documented precedence determines which single response is returned.
- A very long single message content or a very large number of messages: accepted
  as long as the overall request size is within the limit; per-field and
  per-message count limits are outside this phase.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The gateway MUST validate a chat completion request against one
  documented rule set covering `model`, `messages`, `temperature`, `max_tokens`,
  `stream`, and overall request size.
- **FR-002**: `model` MUST be a non-empty string. The gateway MUST NOT require
  the model to be registered or routable in this phase, and MUST preserve the
  submitted value exactly, including surrounding whitespace.
- **FR-003**: `messages` MUST contain at least one entry, MUST preserve the
  submitted order, and each entry MUST have a role of `system`, `user`, or
  `assistant` and a non-empty string `content`.
- **FR-004**: The gateway MUST accept an optional `temperature` control and
  MUST reject values outside the supported range, which is 0.0 to 2.0 inclusive.
- **FR-005**: The gateway MUST accept an optional `max_tokens` control and MUST
  reject values of zero or less and values above the supported maximum of
  4096.
- **FR-006**: When `temperature` or `max_tokens` is absent, the validated
  request MUST record that the control was not specified and MUST NOT present an
  invented value as a value the client supplied.
- **FR-007**: When `temperature` or `max_tokens` is present and valid, the
  validated request MUST carry exactly the submitted value.
- **FR-008**: The gateway MUST reject `temperature` or `max_tokens` supplied as
  a value that is not a number, including a numeric string, boolean, null,
  object, or array, with the documented invalid-request response and MUST NOT
  report a server error for such input.
- **FR-009**: The gateway MUST reject a request that supplies the same control
  more than once, and MUST NOT silently select one occurrence.
- **FR-010**: The gateway MUST reject a request whose body exceeds the supported
  size limit of 1 MB and MUST return a distinct documented oversize response
  rather than a completion or a generic invalid-request response.
- **FR-011**: A request whose size is exactly the supported limit MUST be
  accepted, and the limit MUST apply to the whole request body.
- **FR-012**: The gateway MUST stop working on a request that exceeds the size
  limit, MUST remain healthy afterwards, and MUST continue serving later
  requests.
- **FR-013**: The gateway MUST reject a request that explicitly requests a
  streamed response with the documented unsupported-feature status, code, and
  message, and MUST NOT return a non-streaming completion in its place.
- **FR-014**: The request that enters application logic MUST be a single
  validated value produced by validation, and application logic MUST NOT receive
  an unvalidated request.
- **FR-015**: Validation MUST be deterministic: identical requests MUST produce
  identical outcomes, and a request violating multiple rules MUST produce
  exactly one documented response, chosen by a documented precedence order.
- **FR-016**: The documented precedence order MUST be: route and admission,
  request size, media type, structural readability, required-field validity,
  control-range validity, then streaming rejection.
- **FR-017**: Every validation failure MUST return a response containing exactly
  the documented machine-readable `code` and fixed human-readable `message`.
  This phase MUST NOT add a `details` field or any per-field error list.
- **FR-018**: Validation responses MUST NOT include the submitted value that
  caused a failure, prompt or message content, credentials, parser output, or
  internal diagnostics.
- **FR-019**: The oversize response MUST use the documented code
  `payload_too_large` with the fixed message `The request payload is too large.`
  so that clients can distinguish it from other client errors.
- **FR-020**: Unknown extra fields in a chat request MUST be ignored in this
  phase and MUST NOT cause the request to fail.
- **FR-021**: The gateway MUST continue to require no credential and MUST NOT
  contact any provider, database, cache, or other external service while
  validating or accepting a request.
- **FR-022**: Concurrent validation MUST remain isolated, and one request MUST
  NOT change the result of another request.
- **FR-023**: The existing client-facing error contract for routing, media type,
  invalid request, unsupported feature, not ready, and internal failure MUST
  remain unchanged in status, code, and message.
- **FR-024**: The existing liveness, readiness, and shutdown behavior of the
  running gateway MUST remain unchanged.
- **FR-025**: Existing project regression and quality checks MUST continue to
  pass after this phase, and the project MUST remain buildable, testable, and
  runnable.
- **FR-026**: Automated checks MUST cover every documented validation rule, each
  rule's failure response, the precedence order, and the size limit boundary
  without requiring an external service.
- **FR-027**: User-facing documentation MUST state every validation rule, the
  supported ranges, the size limit, the precedence order, and each failure
  response.

### Client-Facing Validation Contract

| Condition | HTTP Status | Code | Message |
|-----------|-------------|------|---------|
| Missing or invalid `model` | 400 | `invalid_request` | The chat request is invalid. |
| Empty, missing, or invalid `messages` | 400 | `invalid_request` | The chat request is invalid. |
| Unsupported message role | 400 | `invalid_request` | The chat request is invalid. |
| `temperature` or `max_tokens` not a number | 400 | `invalid_request` | The chat request is invalid. |
| `temperature` outside 0.0–2.0 | 400 | `invalid_request` | The chat request is invalid. |
| `max_tokens` outside 1–4096 | 400 | `invalid_request` | The chat request is invalid. |
| Control supplied more than once | 400 | `invalid_request` | The chat request is invalid. |
| Streaming requested | 400 | `unsupported_feature` | Streaming is not supported. |
| Request body larger than 1 MB | 413 | `payload_too_large` | The request payload is too large. |
| Unsupported request media type | 415 | `unsupported_media_type` | The request media type is not supported. |
| Unsupported method on a known path | 405 | `method_not_allowed` | The request method is not allowed for this path. |
| Unknown path | 404 | `not_found` | The requested path was not found. |
| New chat request after shutdown begins | 503 | `not_ready` | The gateway is not accepting new chat requests. |
| Unexpected internal failure | 500 | `internal_error` | The gateway could not complete the request. |

Every row is a top-level object with exactly `code` and `message`. No `details`
field, error wrapper, or per-field list is included. For a `HEAD` request, HTTP
body-suppression semantics apply: the status and headers are returned without a
body.

**Verification (2026-09-30)**: all 14 rows were re-checked against live
responses from a running debug build with no external configuration present.
Rows 1–12 matched their exact status, code, and message byte for byte. Row 14
(`internal_error`) has no wire trigger in this contract, so it is asserted at the
error-mapping layer instead. Row 13 (`not_ready`) could not be caught over a live
socket: the mock gateway reaches `Ready` and drains fast enough that no request
landed inside either the initializing or the shutdown window, and once the
process has fully exited the port refuses connections rather than answering
503. Row 13 is therefore verified deterministically by the automated suite
(`chat_is_rejected_while_not_ready`, `initializing_rejects_chat_with_not_ready_contract`,
`shutting_down_rejects_chat_with_not_ready_contract`, and
`stopped_rejects_chat_with_not_ready_contract`), all of which pass.

### Key Entities

- **Validation Rule**: One documented, testable condition that a chat request
  either satisfies or violates; identified by the field or concern it governs.
- **Validated Chat Request**: The single internal request value that carries a
  model, an ordered message list, and each generation control as either supplied
  with its value or explicitly not supplied. This is the only request form that
  reaches application logic.
- **Generation Control**: An optional client-supplied parameter affecting
  response generation. This phase introduces two: sampling temperature and
  response-length limit. Each is either supplied with a value or unspecified.
- **Message**: One conversation entry with a role of `system`, `user`, or
  `assistant` and non-empty text content.
- **Validation Failure**: A rejected request identified by which documented rule
  it violated, reported to the client only as a status and a fixed safe code and
  message.
- **Request Size Limit**: The maximum accepted whole-request body size, and the
  rule that a request exceeding it is refused before it is validated further.

## Success Criteria *(mandatory)*

### Measurable Outcomes

For repeatable verification, the canonical request uses media type
`application/json`, model `mock-model`, and one `user` message containing
`Hello`. Boundary verification uses a temperature of exactly 0.0 and exactly 2.0
as accepted and 2.0001 as refused, a max_tokens of exactly 1 and exactly 4096 as
accepted and 4097 as refused, and request bodies of exactly 1 MB as accepted and
1 byte more as refused. No provider, credential, or external service is
available during any verification.

- **SC-001**: A developer can read the validation rules from the project
  documentation and predict the response for any request without reading the
  gateway's source code.
- **SC-002**: Every documented validation rule is covered by an automated check
  that asserts the exact status, code, and message, with 100% of the rules
  covered.
- **SC-003**: Across 100 repetitions of the canonical request, 100% return the
  same result and the same response body, and 100% of responses are unaffected by
  the presence or absence of the optional generation controls.
- **SC-004**: 100% of boundary cases behave as documented: 0.0 and 2.0
  temperature are accepted, values outside that range are refused, 1 and 4096
  max_tokens are accepted, 4097 is refused, and a body of exactly 1 MB is
  accepted while one byte more is refused.
- **SC-005**: Across 100 repetitions of each documented failure condition, 100%
  return the specified status, code, and fixed message, and 0% of responses
  contain the rejected value, prompt content, a parser message, a credential, or
  any internal diagnostic.
- **SC-006**: Across 20 repetitions of each multi-rule request used to verify
  precedence, 100% return the same single documented response for the same
  combined violation.
- **SC-007**: No request reaches application logic unless it satisfies every
  documented rule, verified by automated checks covering both accepted and
  refused requests.
- **SC-008**: Concurrent validation of 50 simultaneous requests returns 50
  isolated, correct results, with no request influencing another request's
  result.
- **SC-009**: Liveness and readiness remain available and correct before, during,
  and after validation of valid and invalid requests, and an oversized request
  does not degrade later request handling.
- **SC-010**: The existing regression suite and quality checks pass unchanged
  after this phase, and the gateway still starts and serves all three endpoints.

## Assumptions

- This phase is the roadmap's "Domain Models and Validation" milestone. Its
  deliverable is a validated chat request entering the application layer, and its
  declared validation scope is `model`, `messages`, `temperature`, `max_tokens`,
  `stream`, and request size.
- The gateway's internal request and response models stay provider-agnostic. No
  provider-specific request shape, provider selection, or provider concept
  enters the validated request in this phase.
- The validated request is an internal concept. Clients continue to exchange the
  existing chat request and response documents; no new endpoint, version, or
  client-visible field set is introduced.
- Sampling temperature is supported from 0.0 to 2.0 inclusive, and the response-
  length limit is supported from 1 to 4096 inclusive. These are gateway-level
  bounds chosen for this phase; per-model, per-tenant, and policy-based limits
  belong to the routing and policy phases.
- The request size limit is 1 MB of whole request body. A configurable limit, and
  the security-hardening treatment of size limits, belong to the security phase;
  this phase introduces the fixed bound and its documented response.
- The single 1 MB body bound replaces any implicit transport default for this
  endpoint. It applies to the request body and not to headers.
- Per-field content limits, a maximum message count, and per-message size limits
  are outside this phase and are deferred to the domain-validation follow-up.
  The whole-body limit is the only size rule in this phase.
- Generation controls are validated and carried through, but they do not yet
  change what the gateway returns, because no real model is connected. This phase
  must not present a control as having influenced a response.
- Errors continue to be reported through the existing flat, two-field error
  contract. Identifying the offending field is deliberately not added in this
  phase, so the contract stays stable for existing clients.
- The same control appearing more than once in a request body is ambiguous
  input rather than a last-value-wins override, so it is refused.
- Streaming remains unimplemented. A request that explicitly asks for a stream
  continues to be refused with the existing unsupported-feature response, and the
  validated request records that no stream was requested.
- Temperature and max_tokens remain optional. Omitting them is valid and never
  produces an error.
- Liveness, readiness, routing, media-type handling, and graceful shutdown
  behavior are established in the previous phase and are not changed here.
- The API is still a local, unauthenticated development service. This phase adds
  no authentication, authorization, rate limiting, persistence, caching,
  observability, or provider access.
- The feature depends on the completed foundation, architecture, and HTTP gateway
  core phases and must preserve their behavior.
- The feature remains subject to the project constitution, including incremental
  delivery, explicit boundaries, safe error handling, testability, and the
  requirement that the project stays runnable after the phase.
