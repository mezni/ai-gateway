# Feature Specification: Layered Architecture

**Feature Branch**: `002-layered-architecture`

**Created**: 2026-09-24

**Status**: Draft

**Input**: User description: "read from docs/plan.md phase 1"

## Clarifications

### Session 2026-09-24

- Q: How complete should the initial domain type attributes be in this phase? → A: Option A - essential fields only (model + messages on a chat request, role + content on a message, token counts on usage, id on a model, id + name on a provider); full field sets deferred to their own phases.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Establish Clear Architectural Boundaries (Priority: P1)

A developer working on the gateway can point to distinct, named modules for
each architectural concern — domain, application, infrastructure, API, and
configuration — and knows precisely where a given piece of logic belongs.
The skeleton compiles, so the boundaries are enforced by the project itself,
not just by convention.

**Why this priority**: The constitution mandates explicit boundaries
(Principle V). Every later phase (provider integration, routing, security,
persistence) lands inside one of these layers. Without the skeleton, later
feature work has nowhere clean to attach.

**Independent Test**: Can be fully tested by inspecting the module layout,
confirming each planned layer exists, and running the build. It delivers a
navigable structure that keeps the codebase maintainable as features grow.

**Acceptance Scenarios**:

1. **Given** the project repository, **When** a developer inspects the source
   layout, **Then** distinct modules exist for domain, application,
   infrastructure, API, and configuration concerns.
2. **Given** the architecture skeleton, **When** the developer runs the build,
   **Then** the project compiles successfully.
3. **Given** the documented boundaries, **When** a developer examines where a
   future feature (for example provider integration) should live, **Then**
   the module structure provides an unambiguous home for it.

---

### User Story 2 - Define the Initial Domain Model (Priority: P1)

A developer can work with a shared, provider-independent vocabulary for the
core objects the gateway manipulates: chat requests, messages, message roles,
chat responses, usage, models, and providers. These types are defined once and
used consistently across layers.

**Why this priority**: The domain model is the heart of provider independence
(Principle IV). Defining it before any provider integration guarantees the
gateway's internals never become married to a single provider's format.

**Independent Test**: Can be fully tested by confirming every planned domain
type exists, compiles, and carries the essential fields its purpose implies.
It delivers the shared vocabulary all gateway phases rely on.

**Acceptance Scenarios**:

1. **Given** the architecture skeleton, **When** the domain module is
   inspected, **Then** chat request, message, message role, chat response,
   usage, model, and provider types are all present.
2. **Given** the domain types, **When** they are compiled and exercised by the
   existing test suite, **Then** they build and function without any external
   service.
3. **Given** the domain model, **When** a developer reads it, **Then** no
   provider-specific concepts leak into the types' core meaning.

---

### User Story 3 - Prepare Composition Without Coupling (Priority: P2)

A developer can see how future components will be assembled — application
state and dependency-injection seams — without any real provider being wired
in yet. This proves the architecture supports composition before external
dependencies exist.

**Why this priority**: Dependency injection and application state are the
mechanisms that keep layers decoupled as the gateway grows. Demonstrating the
mechanism this early (Principle I: incremental, runnable at every step) makes
later wiring trivial instead of a refactor.

**Independent Test**: Can be fully tested by confirming the composition
mechanism compiles and that the application still runs with no real provider
dependency present. It delivers the assembly pattern future phases will reuse.

**Acceptance Scenarios**:

1. **Given** the architecture skeleton, **When** a developer reviews the
   composition layer, **Then** there is an explicit mechanism for supplying
   dependencies to components.
2. **Given** the application, **When** it is started, **Then** it runs without
   contacting any external provider.
3. **Given** the composed skeleton, **When** the developer removes any
   placeholder component, **Then** the application still builds, proving no
   hidden couplings exist.

---

### Edge Cases

- What happens when a developer places logic in the wrong layer? The module
  boundaries must be documented clearly enough that the correct placement is
  discoverable (documented in the structure guide).
- What happens if the domain types are empty placeholders? Types must carry
  the essential fields implied by their purpose so they provide real value
  rather than stubs.
- What happens when the skeleton grows before the next phase starts? The
  structure must remain runnable and testable at every step (constitution
  Principle III).
- What happens when a provider-specific idea is needed inside the domain
  model? Provider-specific concerns are out of scope; they belong to provider
  integration phases and must not be introduced here.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The source layout MUST expose distinct modules for domain,
  application, infrastructure, API, and configuration concerns.
- **FR-002**: The architecture skeleton MUST compile successfully.
- **FR-003**: The domain module MUST define types for chat request, message,
  message role, chat response, usage, model, and provider.
- **FR-004**: Each planned domain type MUST carry the essential fields implied
  by its purpose: a chat request carries a model reference and its messages; a
  message carries a role and content; message roles cover the standard system,
  user, and assistant roles; a chat response carries its content and usage;
  usage captures prompt, completion, and total tokens; a model carries an
  identifier; a provider carries an identifier and name. Full attribute
  expansion is deferred to the phases that define request validation and
  provider mapping.
- **FR-005**: The domain model MUST remain provider-independent; no
  provider-specific concepts are introduced in this phase.
- **FR-006**: The skeleton MUST include an explicit mechanism for supplying
  dependencies to components (application state / injection seam), even though
  no real providers exist yet.
- **FR-007**: The application MUST continue to run with no real provider
  dependency (no provider credentials, calls, or configuration required).
- **FR-008**: Existing foundation behavior MUST keep working unchanged: the
  application still produces its banner and exits cleanly.
- **FR-009**: The module boundaries MUST be documented so that developers can
  determine where future features belong.
- **FR-010**: All established quality gates (format, lint, tests, build) MUST
  continue to pass.

### Key Entities

- **ChatRequest**: A client's request for a chat completion; carries a model
  reference and the messages to send.
- **Message**: A single exchange within a chat; carries a role and content.
- **MessageRole**: The role of a message; spans system, user, and assistant.
- **ChatResponse**: The gateway's normalized response to a chat request;
  carries content and usage.
- **Usage**: Token accounting for a request — prompt, completion, and total
  token counts.
- **Model**: A model identifier as understood by the gateway.
- **Provider**: An LLM service the gateway can route to, identified by an
  identifier and name without coupling to any specific vendor.

Essential attributes only are defined in this phase; richer attributes arrive
with the request-validation and provider-mapping phases.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the planned architectural layers (domain, application,
  infrastructure, API, configuration) exist as distinct modules and compile.
- **SC-002**: 100% of the planned domain types exist and are exercised by the
  test suite without any external service.
- **SC-003**: The application starts and exits cleanly with no provider
  credentials or external calls configured, verified by running it end to end.
- **SC-004**: 100% of the foundation regression suite passes (existing tests
  unchanged and green).
- **SC-005**: All quality gates pass consistently on repeated runs.
- **SC-006**: A developer can determine where a future feature belongs from
  the documented structure without asking a teammate.

## Assumptions

- The layer boundaries follow the target structure already documented in the
  project's architecture documents (`docs/architecture.md`, `docs/plan.md`).
- Domain types are defined at the level of essential fields; full request
  validation and provider-specific mapping are deliberately deferred to their
  own later phases.
- No new dependencies (frameworks, provider SDKs, persistence) are introduced
  in this phase beyond what the scaffolding requires.
- Provider integration, model routing, and the HTTP API are explicit
  out-of-scope for this phase and arrive in subsequent specifications.
- The phase respects the constitution: explicit boundaries (V), provider
  independence (IV), incremental development (III), and testability (IX) take
  priority over convenience shortcuts.
- The architecture skeleton must remain runnable after every change
  (Principle III).
- The existing `app_version()` foundation API stays intact; later phases may
  move it internally but must preserve the application's observable behavior.