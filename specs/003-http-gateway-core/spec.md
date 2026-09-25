# Feature Specification: HTTP Gateway Core

**Feature Branch**: Not provided

**Created**: 2026-09-25

**Status**: Draft

**Input**: User description: "read from docs/plan.md phase 2"

## Overview

This phase turns the gateway into a long-running local service. Operators can
tell whether it is alive and ready for traffic, while application developers can
submit a chat request and receive a fixed sample answer. The milestone requires
no account, provider credential, or external service. Real model access, sign-in,
streaming, routing, and advanced request controls remain later capabilities.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Confirm Gateway Liveness (Priority: P1)

An operator or monitoring client calls `GET /health` to determine whether the
gateway process is alive and able to answer HTTP requests. The response is quick,
does not require credentials, and does not depend on any external service.

**Why this priority**: A reliable liveness signal is the minimum operational
contract needed to manage a long-running gateway. It is the first independently
useful slice of the HTTP feature.

**Independent Test**: Start the gateway without external services, call
`GET /health`, and verify that a live gateway returns the documented successful
status response. This delivers a working liveness check on its own.

**Acceptance Scenarios**:

1. **Given** a running gateway that can answer requests, **When** a client calls
   `GET /health`, **Then** the gateway returns HTTP 200 with a response whose
   status is `ok`.
2. **Given** no provider, database, cache, or other external service is
   configured, **When** a client calls `GET /health`, **Then** the check still
   succeeds without credentials or remote calls.
3. **Given** repeated liveness checks, **When** clients call the endpoint
   concurrently, **Then** each request receives the same successful result and
   one request cannot alter another request's result.

---

### User Story 2 - Determine Traffic Readiness (Priority: P1)

An operator calls `GET /ready` before sending client traffic to determine
whether startup has completed and the gateway is able to accept requests. Liveness
and readiness remain separate signals so a live process is not mistaken for a
process ready for traffic.

**Why this priority**: Reliable traffic management depends on distinguishing a
live process from one that can safely receive work. This is independently useful
and necessary before exposing the chat capability.

**Independent Test**: Start the gateway, call `GET /ready`, and verify that it
returns the documented ready result only after the gateway can accept traffic. A
controlled startup scenario can verify the not-ready result without requiring a
real external service.

**Acceptance Scenarios**:

1. **Given** startup has completed and the gateway can accept traffic, **When** a
   client calls `GET /ready`, **Then** the gateway returns HTTP 200 with a
   response whose status is `ready`.
2. **Given** the process can respond but is not yet able to accept traffic,
   **When** a client calls `GET /ready`, **Then** the gateway returns HTTP 503
   with a response whose status is `not_ready` and no sensitive diagnostic
   details.
3. **Given** a live but not-ready gateway, **When** liveness and readiness are
   checked separately, **Then** their results communicate the different states
   rather than returning the same signal for both purposes.
4. **Given** the configured network address or port cannot be used, **When** the
   gateway starts, **Then** it exits unsuccessfully with a message identifying
   the address or port problem and never returns a ready result.
5. **Given** no provider, database, cache, or other external service exists,
   **When** readiness is checked, **Then** the result reports only the gateway's
   ability to accept traffic and makes no claim about those services.
6. **Given** the gateway begins shutting down, **When** readiness is checked,
   **Then** the gateway returns HTTP 503 with status `not_ready`, rejects new
   chat requests with HTTP 503, and gives in-flight mock requests at most 10
   seconds to finish before the process exits.

---

### User Story 3 - Exercise a Mock Chat Completion (Priority: P1)

An application developer calls `POST /v1/chat/completions` with a gateway model
name and one or more messages. The gateway accepts the request without external
credentials, returns a deterministic mocked assistant response, and proves the
complete HTTP request path works before a real model provider is connected.

**Why this priority**: This is the phase's primary user journey and the first
end-to-end gateway slice visible to clients. It converts the existing chat
vocabulary into a working HTTP capability while keeping external dependencies out
of scope.

**Independent Test**: Start the gateway with no provider configuration, send a
valid chat completion request, and verify that the response contains one mocked
assistant choice, echoes the requested model, and requires no external service.
This delivers a complete mock chat interaction on its own.

**Acceptance Scenarios**:

1. **Given** an `application/json` request containing a non-empty model
   identifier and at least one message whose role is `system`, `user`, or
   `assistant` and whose content is non-empty text, **When** a client calls
   `POST /v1/chat/completions`, **Then** the gateway returns HTTP 200 with one
   non-streaming assistant choice whose content is
   `This is a mocked chat completion.` and echoes the requested model.
2. **Given** the same valid request is submitted more than once, **When** each
   request is processed, **Then** every response has the same mocked content and
   status so the behavior is deterministic.
3. **Given** no provider credentials are configured, **When** a valid chat
   completion request is processed, **Then** the gateway returns the mock
   response without contacting an external service.
4. **Given** a client explicitly requests streaming, **When** the request is
   received, **Then** the gateway returns HTTP 400 with error code
   `unsupported_feature` rather than silently returning a non-streaming result.
5. **Given** the mocked response, **When** a client inspects it, **Then** the
   response contains no token-usage field and does not present invented usage
   values as real model usage.
6. **Given** a non-empty model identifier that has no configured route,
   **When** the client sends the chat request, **Then** the mock phase still
   accepts and echoes it without claiming that provider selection occurred.
7. **Given** the model or required message information is missing, **When** a
   client submits the request, **Then** the gateway returns HTTP 400 with error
   code `invalid_request` and the safe message `The chat request is invalid.`
8. **Given** two valid requests with different models and messages are submitted
   concurrently, **When** both complete, **Then** each response contains only
   its own requested model and no content from the other request.

---

### User Story 4 - Receive Predictable API Errors (Priority: P2)

An application developer receives a consistent, safe error for unsupported or
malformed requests. The error identifies the problem in a machine-readable way
without revealing internal implementation or sensitive information.

**Why this priority**: Predictable errors make the HTTP contract safe to integrate
and test. This is important but secondary to proving the successful gateway
journeys.

**Independent Test**: Exercise each supported client-error scenario without an
external service and verify the status category, error code, and absence of
sensitive details. This delivers a trustworthy failure contract on its own.

**Acceptance Scenarios**:

1. **Given** a malformed or structurally unreadable chat request, **When** a
   client submits it, **Then** the gateway returns HTTP 400 with error code
   `invalid_request` and message `The chat request is invalid.`
2. **Given** a request body uses an unsupported media type, **When** a client
   submits it, **Then** the gateway returns HTTP 415 with error code
   `unsupported_media_type` and message
   `The request media type is not supported.`
3. **Given** a known path is called with an unsupported method, **When** a client
   submits the request, **Then** the gateway returns HTTP 405 with error code
   `method_not_allowed` and message
   `The request method is not allowed for this path.`. For a `HEAD` request,
   HTTP body-suppression semantics apply, so the status and headers are returned
   without a response body.
4. **Given** an unknown path, **When** a client requests it, **Then** the gateway
   returns HTTP 404 with error code `not_found` and message
   `The requested path was not found.`.
5. **Given** an unexpected internal failure, **When** the gateway cannot fulfill
   a request, **Then** it returns HTTP 500 with error code `internal_error`,
   message `The gateway could not complete the request.`, and no credentials,
   internal stack details, or implementation-specific diagnostics.

### Edge Cases

- The configured network address or port is unavailable: startup fails and the
  message identifies whether the address or port is the problem.
- A client sends an empty body, malformed structured data, or a body that cannot
  be read: the gateway returns the `invalid_request` error contract.
- A chat request omits the minimum model or message information needed for this
  phase: the gateway returns HTTP 400 with `invalid_request`; broader domain
  validation remains out of scope.
- A request asks for streaming: the gateway returns the `unsupported_feature`
  error contract.
- A client disconnects before a response is complete: the gateway stops work that
  is no longer needed and remains healthy for later requests.
- Several clients submit chat requests concurrently: responses remain isolated
  and deterministic, with no content from one request appearing in another.
- The gateway begins shutting down: health remains available, readiness reports
  `not_ready`, new chat requests receive `not_ready`, and in-flight mock requests
  receive at most 10 seconds to finish.
- A critical startup condition is invalid: the gateway fails fast rather than
  starting in a deceptively ready state.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The gateway MUST expose `GET /health`, `GET /ready`, and
  `POST /v1/chat/completions` as its initial HTTP contract.
- **FR-002**: The liveness endpoint MUST return HTTP 200 and status `ok` whenever
  the running gateway can answer requests.
- **FR-003**: The liveness endpoint MUST NOT require credentials or depend on the
  availability of an external service.
- **FR-004**: The readiness endpoint MUST return HTTP 200 and status `ready` only
  after startup has completed and the gateway can accept traffic.
- **FR-005**: The readiness endpoint MUST return HTTP 503 and status `not_ready`
  whenever the gateway can respond but is not ready to accept traffic.
- **FR-006**: Readiness MUST NOT claim that unconfigured or future external
  dependencies are healthy.
- **FR-007**: The chat completion endpoint MUST accept `application/json` request
  bodies containing a non-empty model identifier and at least one message whose
  role is `system`, `user`, or `assistant` and whose content is non-empty text.
  The mock phase MUST NOT require the model to be registered or routable.
- **FR-008**: A valid chat completion request MUST return HTTP 200, exactly one
  mocked assistant choice with content `This is a mocked chat completion.`, the
  requested model identifier, and a non-streaming response.
- **FR-009**: The mocked chat response MUST be deterministic for identical valid
  requests and MUST NOT require provider credentials or contact an external
  service.
- **FR-010**: The mocked chat response MUST omit any token-usage field rather than
  claim that unavailable usage was measured.
- **FR-011**: A request that explicitly asks for streaming MUST receive HTTP 400
  with error code `unsupported_feature` and message
  `Streaming is not supported.`
- **FR-012**: Malformed, structurally unreadable, or minimum-contract-violating
  request bodies MUST receive HTTP 400 with `invalid_request`; unsupported media
  types MUST receive HTTP 415 with `unsupported_media_type`; unsupported methods
  on known paths MUST receive HTTP 405 with `method_not_allowed`; and unknown
  paths MUST receive HTTP 404 with `not_found`.
- **FR-013**: Every request-processing error MUST contain exactly the
  machine-readable `code` and fixed human-readable `message` defined by the
  error contract. This phase MUST NOT include a `details` field. For `HEAD`
  requests, HTTP body-suppression semantics apply: the corresponding status and
  headers MUST be returned without a response body.
- **FR-014**: Unexpected failures MUST return HTTP 500 with error code
  `internal_error`, its fixed safe message, and no secrets or internal
  diagnostics.
- **FR-015**: Concurrent requests MUST remain isolated, and one request MUST NOT
  change the result or content of another request.
- **FR-016**: When shutdown begins, the gateway MUST continue answering liveness
  and readiness, MUST report readiness as `not_ready`, MUST reject new chat
  requests with HTTP 503 and code `not_ready`, and MUST allow in-flight mock
  requests at most 10 seconds to finish before the process exits.
- **FR-017**: Critical startup configuration MUST be validated before the gateway
  reports ready. The documented local default MUST be address `127.0.0.1` and port
  `3000`, and both values MUST be overridable through runtime configuration;
  invalid values MUST cause a clear unsuccessful startup.
- **FR-018**: Existing project regression and quality checks MUST continue to
  pass after the gateway changes the application from one-shot execution to a
  long-running service.
- **FR-019**: Automated checks MUST cover the successful and primary failure
  behavior of all three endpoints without requiring an external service.
- **FR-020**: User-facing documentation MUST explain how to start the gateway,
  exercise each endpoint, interpret health versus readiness, and stop the
  gateway cleanly.

### Client-Facing Error Contract

Request-processing errors use an object with exactly `code` and `message`. The
message text is fixed for this phase, and no `details` field is included.

| Condition | HTTP Status | Code | Message |
|-----------|-------------|------|---------|
| Malformed request or missing minimum fields | 400 | `invalid_request` | The chat request is invalid. |
| Streaming requested | 400 | `unsupported_feature` | Streaming is not supported. |
| Unsupported request media type | 415 | `unsupported_media_type` | The request media type is not supported. |
| Unsupported method for a known path | 405 | `method_not_allowed` | The request method is not allowed for this path. |
| Unknown path | 404 | `not_found` | The requested path was not found. |
| New chat request after shutdown begins | 503 | `not_ready` | The gateway is not accepting new chat requests. |
| Unexpected internal failure | 500 | `internal_error` | The gateway could not complete the request. |

Liveness and readiness are operational status responses rather than this error
shape: they return a `status` value as defined in their requirements. For `HEAD`
requests, the selected status and headers are returned without a body, as
required by HTTP semantics.

### Key Entities

- **Liveness Status**: The gateway's answer about whether the running process can
  respond to requests; represented externally as `ok` when healthy.
- **Readiness Status**: The gateway's answer about whether traffic can be
  accepted; represented externally as `ready` or `not_ready`.
- **Chat Completion Request**: A client request containing a gateway model
  identifier and an ordered set of messages for a mock completion.
- **Message**: One conversation entry with a `system`, `user`, or `assistant` role
  and non-empty textual content.
- **Mock Chat Completion**: A successful, deterministic, non-streaming response
  containing the requested model and one assistant choice with content
  `This is a mocked chat completion.`. It represents gateway flow rather than a
  real model response and contains no token-usage field.
- **API Error**: A consistent request-processing failure response containing
  exactly `code` and its fixed safe `message`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

For repeatable verification, the canonical chat request uses media type
`application/json`, model `mock-model`, and one `user` message containing
`Hello`. Its expected result is HTTP 200 with one assistant choice containing
`This is a mocked chat completion.` and no token-usage field. Operational checks
use the documented local address and port with no competing load. Response time
is measured from request dispatch until the complete response is received on the
same machine.

- **SC-001**: A developer starting from a clean checkout can launch the gateway
  and exercise all three endpoints in under 10 minutes using the included
  documentation, with no credentials or external services required.
- **SC-002**: Automated checks cover every acceptance scenario and all seven
  client-facing error-contract rows without network access to a model provider
  or another external service.
- **SC-003**: Across 100 consecutive requests to each of the liveness and
  readiness endpoints, at least 95% return the expected result within 250
  milliseconds and 100% return the correct status.
- **SC-004**: Across 100 repetitions of the canonical chat request, 100% return
  HTTP 200, the exact documented assistant content, the requested model, one
  assistant choice, and no token-usage field.
- **SC-005**: Across each of the seven error-contract conditions, 100% return
  the specified HTTP status, code, and fixed message, and expose no credentials,
  prompts, stack details, or internal diagnostics.
- **SC-006**: Across 20 repetitions of each reachable state—initializing, ready,
  and shutting down—100% produce the expected liveness and readiness pair:
  `ok`/`not_ready`, `ok`/`ready`, and `ok`/`not_ready`, respectively. After exit,
  the gateway answers no further requests.
- **SC-007**: After a clean shutdown request, no new chat request is accepted,
  in-flight mock requests finish or the process exits within 10 seconds, no
  gateway process or listening socket remains, and the existing regression suite
  is green before shutdown begins.
- **SC-008**: The primary user journey can be demonstrated end to end with only
  the local gateway; no model-provider setup or credential exchange is needed.

## Assumptions

- Phase 2 is a local development milestone. The three endpoints are intentionally
  unauthenticated until the authentication phase, and the gateway must not be
  exposed to untrusted traffic without the protections specified by later phases.
- The chat response is a deterministic product mock that proves the gateway's
  request and response journey; it is not an LLM response and does not report
  measured token usage.
- Existing chat concepts, including model, message, role, and response, remain the
  shared vocabulary for this phase. Any non-empty model identifier is accepted
  by the mock, and their fuller validation and routing rules belong to later
  phases.
- Only the minimum request shape is enforced here. Temperature, token limits,
  request-size limits, and comprehensive field validation are deferred to the
  domain-validation phase.
- Provider access, provider abstraction, model selection, routing,
  authentication, authorization, rate limiting, retries, fallback, streaming,
  persistence, caching, and production observability are outside this phase.
- Health and readiness are public operational endpoints in this phase. Readiness
  covers the gateway's own ability to serve traffic and does not represent the
  health of dependencies introduced in later phases.
- The gateway defaults to `127.0.0.1:3000` for local use and allows its listening
  address and port to be overridden through runtime configuration.
- The broader API design describes the target gateway. This specification defines
  the phase 2 subset: authentication, provider routing, streaming, and advanced
  validation described there are not active until their later specifications.
- No durable data is created by this phase; operational information is available
  only while the process is running.
- This feature depends on the completed project foundation and layered
  architecture. It must preserve their existing behavior and leave the project
  buildable, testable, and runnable.
- The feature remains subject to the project constitution, including incremental
  delivery, clear boundaries, safe error handling, testability, and graceful
  shutdown.
