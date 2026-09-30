# Feature Specification: Provider Abstraction

**Feature Branch**: `005-provider-abstraction`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: "read from docs/plan.md phase 4" — Phase 4, Provider Abstraction (`docs/plan.md` section 9)

## Overview

The gateway currently answers every chat request from a single hard-coded
completion. Nothing in it knows that a response can come from somewhere else,
so no second source of completions can be added without rewriting the request
path.

This feature introduces a provider boundary: a single, provider-agnostic way
for the gateway to ask "produce a completion for this validated chat", with
each provider's own request and response format confined to that provider
alone. The gateway gains the ability to be pointed at a real large language
model service while its own concepts stay unchanged, without that service being
connected yet.

Adding this boundary is what makes every later capability — real provider
connectivity, model routing, fallbacks, usage tracking, cost accounting —
possible without revisiting the request path. It is the structural
prerequisite, not a user-visible capability on its own.

**Why this phase exists now**: constitution principle III (Incremental
Architecture) requires each phase to leave the system runnable, and principle
IV (Provider Independence) requires providers be reached only through explicit
abstractions with provider-specific formats isolated in adapters. Both are
currently unmet for the completion path.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Gateway Operators Route a Request to a Chosen Provider (Priority: P1)

An operator runs the gateway and wants a chat request to be answered by the
provider of their choice. They select that choice through configuration at
startup. They do not edit code, and they do not need to restart into a
different build to change providers.

**Why this priority**: This is the entire point of the feature. Without a
selectable provider there is no abstraction in practice — only a renamed mock.

**Independent Test**: Start the gateway configured for the built-in
deterministic provider, submit a valid chat request, and confirm the response
is served by that provider. Then configure a different provider and confirm
the same request shape is served by it, with no gateway code change.

**Acceptance Scenarios**:

1. **Given** a gateway configured for one provider, **When** a valid chat
   request is submitted, **Then** the response comes from that provider and
   carries the gateway's normal provider-agnostic response shape.
2. **Given** a gateway configured for a provider that is not available or not
   enabled, **When** the gateway starts, **Then** the operator is told which
   provider was requested and that it is unusable, rather than the gateway
   starting in a state where every request fails.
3. **Given** a provider is selected at startup, **When** the operator inspects
   the running gateway, **Then** the effective selection is discoverable
   without reading the source.

---

### User Story 2 - Client Applications See One Stable Response Shape (Priority: P1)

An application built against the gateway should not change when the provider
behind it changes. A provider that returns extra fields, renames fields, or
reports usage differently must still produce the same response to the client.
A provider that fails must fail in a way the client can recognise and handle,
rather than as an unexplained error or a hang.

**Why this priority**: Provider independence is the feature's reason to exist.
If provider quirks leak to clients, the abstraction has failed even if a trait
was introduced.

**Independent Test**: Run the same client request against two different
providers and assert the response bodies are byte-identical for an equivalent
completion. Then force a provider failure and assert the client receives the
gateway's documented error contract, not a provider-specific body.

**Acceptance Scenarios**:

1. **Given** two providers that return materially different native responses,
   **When** the same chat request is sent to each, **Then** the client-visible
   response is identical in shape and field set.
2. **Given** a provider that fails, **When** the failure reaches the client,
   **Then** the client receives the gateway's documented error format and
   never a raw provider error body, message, or credential.
3. **Given** a provider that returns usage information the gateway cannot
   interpret, **When** the response is translated, **Then** the gateway does
   not invent token counts and does not report a malformed success to the
   client.

---

### User Story 3 - Gateway Developers Add a Provider Without Touching the Gateway (Priority: P2)

A developer adds support for a new large language model service. They should
write one self-contained adapter and nothing else. Request handling,
validation, and response construction must not need edits, and a mistake in
the new adapter must not be able to break unrelated behaviour.

**Why this priority**: It is the durability test of the abstraction and the
maintenance payoff, but a single working provider already delivers the primary
user value.

**Independent Test**: Add a test-only provider that returns a fixed response,
confirm the gateway serves it through the normal path, then confirm removing
it changes no other file.

**Acceptance Scenarios**:

1. **Given** a newly added provider, **When** it is registered as a selectable
   option, **Then** no change is required to request validation, to the
   client-facing response, or to any unrelated provider.
2. **Given** a provider that misbehaves, **When** it is exercised, **Then** the
   failure is contained to that provider and does not affect concurrent
   requests served by other providers.
3. **Given** the built-in deterministic provider, **When** a chat request is
   served, **Then** the response is byte-for-byte identical to the behaviour
   the gateway provides today, so no existing client observes a change.

---

### Edge Cases

- The gateway is asked for a provider name that does not exist, or for a
  provider that exists but is disabled.
- A provider is selected but its required credentials are absent from the
  environment at startup.
- A provider returns a success status with a body that is not valid, or is
  valid but missing the completion entirely.
- A provider returns usage figures the gateway cannot reconcile, or reports
  no usage at all.
- A provider is extremely slow, or stops responding mid-request.
- A provider is reachable but refuses the request (for example, an unknown
  model or a rejected credential).
- Two or more requests reach different providers at the same time.
- A request is cancelled or times out while the provider is still working.
- The same request is submitted repeatedly; responses must be repeatable for
  the deterministic provider and free of cross-request state.
- A provider emits an enormous or malformed native response.

## Requirements *(mandatory)*

### Provider Boundary

- **FR-001**: The gateway MUST reach every large language model provider
  through a single provider-agnostic contract that accepts a validated,
  provider-agnostic chat request and returns either a provider-agnostic
  completion or a categorized failure.
- **FR-002**: That contract MUST support being awaited by concurrent requests
  without blocking the calling request, and MUST be usable through shared
  ownership by many concurrent requests.
- **FR-003**: The provider contract MUST carry no provider-specific types in
  its signature. Provider-native request and response structures MUST NOT
  appear in it.
- **FR-004**: The gateway's internal request, completion, and usage models
  MUST remain provider-agnostic and MUST NOT gain provider-specific fields to
  accommodate a single provider.

### Provider Selection

- **FR-005**: The provider used for a request MUST be selectable from
  configuration, chosen at startup.
- **FR-006**: The gateway MUST support a built-in deterministic provider that
  requires no network access, no credentials, and no external configuration.
  This provider is the only provider the gateway ships enabled by default, and
  it is the regression baseline for the client-facing contract.
- **FR-006a**: The provider registry MUST accept more than one provider, and
  the boundary MUST be provable against a second provider without a second
  built-in production provider. Because FR-026 excludes live providers, the
  second provider used to demonstrate the boundary MUST be a test double that
  is registered in tests only and MUST NOT be reachable in a shipped
  configuration. FR-021, SC-001, SC-002, and SC-005 are verified against it.
- **FR-007**: A configuration naming a provider that is unknown, or that is
  known but not enabled, MUST be refused at startup with a message naming the
  requested provider. The gateway MUST NOT start in a state where every request
  fails because of an unusable selection.
- **FR-008**: The effective provider selection MUST be discoverable from the
  running gateway without reading source code.
- **FR-009**: Selecting a provider MUST NOT require editing gateway code or
  rebuilding it.

### Failure Handling

- **FR-010**: The provider contract MUST distinguish failure categories at
  minimum as: provider unreachable, provider refused the request, provider
  responded with something unusable, provider response was invalid, and the
  call exceeded its deadline. Callers MUST be able to branch on the category
  without inspecting a message string.
- **FR-011**: Each provider call MUST have a bounded deadline. A provider that
  never responds MUST NOT hold a client request open indefinitely.
- **FR-012**: A provider failure MUST be translated into the gateway's
  documented client-facing error contract. Raw provider status codes, provider
  message text, and provider credentials MUST NOT reach a client.
- **FR-013**: Provider failure MUST NOT be reported to a client as a
  successful completion, and partial or truncated provider output MUST NOT be
  reported as success.
- **FR-014**: Usage or token counts MUST NOT be invented. When a provider
  supplies no usage information, the gateway MUST represent that absence
  explicitly rather than substituting a fabricated count.

### Behavioral Preservation and Isolation

- **FR-015**: The Phase 3 request contract MUST be unchanged by this feature.
  Validation, the error codes and messages for invalid requests, the request
  size limit, control ranges, and the refusal of streaming MUST all behave
  exactly as they do today.
- **FR-016**: The client-facing response for a given completion MUST NOT
  change when the provider behind it changes.
- **FR-017**: Concurrent requests MUST be isolated. A slow or failing provider
  MUST NOT delay or corrupt responses served through another provider, and no
  state may leak between requests.
- **FR-018**: A provider failure MUST be contained: it MUST NOT bring down the
  gateway, and the gateway MUST remain able to serve health, readiness, and
  subsequent requests afterwards.
- **FR-019**: The gateway MUST support graceful shutdown: a request in flight
  when shutdown begins MUST be allowed to finish or be abandoned deliberately,
  and no new request may be accepted for a provider call afterwards.
- **FR-020**: The deterministic provider MUST produce a byte-identical
  completion for identical input, so the gateway's own test suite remains a
  reliable regression net.

### Isolation and Extensibility

- **FR-021**: Adding a provider MUST require implementing exactly one
  self-contained adapter. It MUST NOT require changes to request validation,
  to the client-facing response, or to any other provider.
- **FR-022**: Provider-specific request and response formats MUST be
  confined to that provider's adapter and MUST NOT leak into request handling,
  routing, or the client-facing API.
- **FR-023**: A provider's credentials MUST be read from the environment and
  MUST NOT be hard-coded, logged, or included in any response or error.
- **FR-024**: The gateway MUST remain provider-independent with respect to its
  domain and client-facing models, so that adding a second real provider does
  not change the client contract.

### Scope Boundary for This Phase

- **FR-025**: This feature MUST deliver the provider boundary, provider
  selection, failure categorization, and a working built-in deterministic
  provider.
- **FR-026**: This feature MUST NOT include live connectivity to a hosted
  large language model service. `docs/plan.md` section 9's deliverable line
  ("the gateway can communicate with OpenRouter through the provider
  abstraction") is superseded by section 10, which assigns OpenRouter
  integration, its configuration block, and its request and response mapping to
  Phase 5. What Phase 4 delivers is the boundary that makes that possible.
- **FR-027**: This feature MUST extend the client-facing error contract with
  two provider-facing codes, bringing it from eight codes to ten: `502
  provider_unavailable` for a provider that cannot be reached or refuses the
  request, and `504 provider_timeout` for a provider call that exceeds its
  deadline. Every other failure category this feature introduces MUST map onto
  one of these two codes or onto the existing `500 internal_error`. No provider
  failure may be reported as a validation failure. The messages for the two new
  codes MUST be fixed strings, and the extension MUST NOT add a `details`
  field, a wrapper object, or a per-category list.
- **FR-028**: Provider selection MUST be configured through process environment
  variables only, consistent with the existing `AI_GATEWAY_HOST` and
  `AI_GATEWAY_PORT` pattern. This feature MUST NOT introduce a configuration
  file or a configuration serialization format. Provider credentials MUST come
  from a named environment variable rather than from a nested structure.
- **FR-029**: This phase MUST NOT add authentication, authorization, rate
  limiting, model routing or fallback, streaming, usage or cost accounting,
  persistence, or observability. Those remain with their own phases.

### Key Entities

- **Provider**: A source of chat completions the gateway can be pointed at.
  Attributes: a name used to select it, whether it is enabled, and whatever
  configuration it needs to be usable. Two exist at the end of this phase: a
  built-in deterministic provider and one other selectable provider.
- **Provider Completion Request**: The provider-agnostic description of what
  to produce — the model, the ordered messages, and the optional generation
  controls. Carries no provider-native fields.
- **Provider Completion**: The provider-agnostic result — an identifier, the
  model that answered, the ordered choices with their role and content, and
  usage. Usage may be explicitly absent.
- **Provider Failure**: A categorized outcome that is not a completion.
  Distinguishes unreachable, refused, unusable response, invalid response, and
  deadline exceeded. Carries no client-facing message text.
- **Provider Selection**: The configured choice of which provider serves
  requests, resolved once at startup.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A second provider can be added and made selectable by changing
  only its own adapter and the selection setting; no other file requires an
  edit, verified by adding a test-only provider and removing it.
- **SC-002**: For an equivalent completion, the client-visible response is
  byte-identical across providers, verified for 100% of a comparison set of
  requests routed to each provider.
- **SC-003**: 100% of provider failure categories are translated into the
  gateway's documented client-facing error contract, with zero occurrences of
  provider message text, provider status codes, or credentials in any response
  body.
- **SC-004**: A provider that never responds results in the client receiving a
  bounded failure in every case, with no request outstanding beyond the
  configured deadline.
- **SC-005**: Under 50 concurrent requests spread across providers, every
  request receives its correct provider's response, with zero cross-provider
  contamination and zero requests served by the wrong provider.
- **SC-006**: 100% of the existing request-contract behaviour continues to
  hold after this feature, with no previously passing check regressing and no
  client-visible change to the responses the gateway returns today.
- **SC-007**: The gateway starts successfully with valid configuration and
  refuses to start, naming the provider, when the selection is unusable, in
  100% of attempts.
- **SC-008**: No provider-specific type, field, or message appears in the
  gateway's client-facing request or response contract, confirmed by review of
  the contract against every provider's native format.
- **SC-009**: A provider failure does not prevent the gateway from serving
  health, readiness, and further successful requests; verified by continuing to
  serve 100% of subsequent requests after each induced failure.

## Assumptions

- **Phase 3 is a stable foundation.** The nine-stage validation pipeline, the
  1 MiB body limit, the inclusive control ranges, the refusal of streaming, and
  the eight error codes are treated as fixed and out of scope to change. Only
  additive changes permitted by FR-027 are in scope.
- **The built-in deterministic provider is the regression baseline.** It
  remains available with no credentials and no network, so the gateway can
  always be run and tested in isolation.
- **The gateway keeps serving exactly three routes.** No new client-facing
  endpoint is added by this feature; provider observability is exposed through
  existing health and readiness surfaces only.
- **A provider is a long-lived, shared, concurrently-accessed component.**
  Selection happens once at startup, not per request, and per-request selection
  is out of scope until model routing arrives.
- **No request may be silently downgraded.** A provider failure surfaces as a
  failure; the gateway does not substitute a different provider unless a later
  phase adds governed fallback.
- **Usage absence is legitimate.** Providers that report no usage are normal,
  and the gateway represents that rather than fabricating counts.
- **Existing delivery discipline carries over.** Every requirement is expected
  to be traceable to an acceptance scenario and to an automated check, and the
  existing quality gates continue to pass unchanged.
- **Feature numbering is sequential.** This feature is `005`, following
  `004-domain-validation`.

## Out of Scope

- Live connectivity to a hosted large language model service, including its
  request and response mapping, deferred to Phase 5 by FR-026.
- Choosing a provider per request, load balancing across providers, and
  governed fallback on failure — Phase 6, Model Registry and Routing.
- Authentication and authorization — Phases 7 and 8.
- Rate limiting — Phase 9.
- Streaming responses — Phase 11.
- Token, cost, and usage accounting beyond representing usage presence — Phase
  13 and Phase 14.
- Persistence, semantic caching, multi-tenancy, and administrative
  configuration.
- Provider-specific capabilities that only one provider offers, including
  provider-native parameters, provider-specific streaming formats, and
  provider-native tool or function calling.

## Dependencies

- `specs/004-domain-validation` — the validated request contract this feature
  consumes, and the error contract FR-012 must map into.
- `specs/002-layered-architecture/contracts/layout.md` — the layering rules
  that place provider implementations in the infrastructure layer and forbid
  provider logic from reaching the API or routing layers.
- `.specify/memory/constitution.md` — principles III (Incremental
  Architecture), IV (Provider Independence), V (Explicit Boundaries), VI
  (Reliability and Failure Isolation), and VII (Security and Privacy by
  Design).
