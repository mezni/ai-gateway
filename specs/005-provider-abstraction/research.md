# Phase 0 Research: Provider Abstraction

**Feature**: `specs/005-provider-abstraction`
**Date**: 2026-09-30
**Purpose**: Resolve the design unknowns behind the specification before
Phase 1 produces the data model and contracts.

Every decision below was checked against the pinned toolchain
(Rust 1.98.1, Edition 2024) or against the existing codebase. Where a claim
could be verified by compiling, it was compiled rather than assumed, and the
result is recorded including the cases that contradicted the initial plan.

---

## D-001: Where the provider contract and its error type live

**Decision**: The provider trait and the provider failure type live in
`src/application/chat.rs`, beside the existing service boundary. Provider
implementations live under `src/infrastructure/providers/`.

**Rationale**: `specs/002-layered-architecture/contracts/layout.md` rule 4
forbids the `application` layer from importing `infrastructure`. The layer
that must *call* a provider is the application layer, so a trait defined in
`infrastructure` would be unreachable from the code that needs it. Rule 8 of
the same contract already states that provider implementations belong in
`infrastructure` and that the domain keeps the provider as a concept only.
The contract is therefore placed to satisfy both rules at once rather than to
satisfy one and break the other.

**Alternatives considered**:
- *Trait in `domain`*: rejected. `domain` must stay free of async, transport,
  and serialization concerns, and a provider contract is an outbound
  integration seam, not a domain concept.
- *Trait in `api`*: rejected outright by layout rule 3, which forbids the API
  layer from importing `domain` types and requires transport concerns to stay
  in the transport layer.

**Consequence**: `infrastructure.rs` stops being an empty placeholder, which
layout rule 9 already anticipated for the phase that first needs adapters.

---

## D-002: How the async provider trait is expressed

**Decision**: Use the `async-trait` proc-macro crate (verified resolvable at
`async-trait` 0.1.92).

**Rationale**: This corrects an assumption in the first draft of
[plan.md](plan.md), which proposed using native `async fn` in traits to avoid
a dependency. That assumption was tested and is **false for this design**.
RPITIT is not dyn-compatible: compiling a trait whose method returns
`impl Future` and then attempting to store an implementation as
`Arc<dyn Trait>` fails with `the trait is not dyn compatible`. Provider
selection requires a registry holding heterogeneous providers behind a single
type, so `dyn` dispatch is not optional here.

`docs/plan.md` section 9 additionally specifies the trait with an explicit
`#[async_trait]` attribute and lists "async traits" as a learning objective,
so this matches the project's own stated intent.

**Alternatives considered**:
- *Hand-written boxed future*, i.e. declaring
  `fn chat(&self, r: &Req) -> Pin<Box<dyn Future<Output = ...> + Send + '_>>`.
  Compiled and verified to work with `dyn` dispatch on this toolchain, so it is
  a genuine alternative rather than a hypothetical. Rejected because it pushes
  `Box::pin(async move { ... })` boilerplate onto every adapter, including the
  test doubles, and the error messages from a failed trait bound are worse.
  Its one concrete advantage is no proc-macro dependency. Recorded here rather
  than dismissed, because a future phase adding many adapters could revisit it.
- *Enum-based dispatch instead of `dyn`*, with one variant per provider.
  Rejected because FR-021 requires adding a provider without editing unrelated
  components, and an enum variant is exactly such an edit.

**Consequence**: One new dependency. The project already depends on proc-macro
crates transitively through `serde` and `axum`'s derive support, and the
constitution does not prohibit dependencies; it prohibits infrastructure
arriving before a specification requires it, which this does.

---

## D-003: Provider failure categories and their client-facing codes

**Decision**: Five categories, mapped as follows.

| Category | Meaning | Client-facing code |
|---|---|---|
| `Unreachable` | Connection could not be established, or the provider could not be contacted | `502 provider_unavailable` |
| `Refused` | Provider was reached and declined the request | `502 provider_unavailable` |
| `DeadlineExceeded` | The call exceeded the configured deadline | `504 provider_timeout` |
| `UnusableResponse` | Provider returned success but the body could not be interpreted | `500 internal_error` |
| `InvalidResponse` | Provider returned an interpretable body with no completion in it | `500 internal_error` |

**Rationale**: FR-010 requires at least these five distinctions and requires
callers to branch on the category rather than on a message string, so the enum
is the branching surface. FR-027 authorises exactly two new client-facing
codes, so the mapping above is constrained: a provider failure that is not a
reachability problem or a timeout is the gateway's own inability to make sense
of a provider, which is what `internal_error` already means. Critically, no
category maps to a validation code, which FR-027 forbids and which would
otherwise blame the client for a provider fault.

`docs/api.md` section 20 already listed 502 and 504 as planned statuses with
the meanings used here, so this fills a documented gap rather than inventing
one.

**Fixed messages**, matching the sentence style of the existing eight codes:
- `502 provider_unavailable` → `The upstream provider is unavailable.`
- `504 provider_timeout` → `The upstream provider did not respond in time.`

Both are fixed strings with no interpolation, per FR-027 and constitution
"Internal errors MUST NOT unnecessarily expose secrets, credentials, stack
details, or sensitive provider information to API clients."

**Alternatives considered**:
- *Collapsing all five onto `500`*: rejected during clarification. It reports a
  provider outage as a gateway bug, which misleads both operators and clients.
- *One code per category (five new codes)*: rejected. FR-027 permits two, and
  distinguishing "unusable response" from "invalid response" to a client is
  not actionable.

---

## D-004: Where the provider deadline is enforced

**Decision**: The deadline is enforced in the **application layer**, wrapping
the provider call, not inside each adapter. It is read from the environment at
startup and validated then.

**Rationale**: Constitution principle VI requires bounded handling of provider
timeouts, and FR-011 requires a bounded deadline. If enforcement lived in each
adapter, an adapter could forget it, and the guarantee would hold only for
adapters that happen to implement it. Enforcing it centrally makes FR-011 a
property of the gateway rather than a convention for adapter authors. It also
means the test double is bounded by the same mechanism as a real provider, so
timeout behaviour is testable without a network.

**Environment variable**: `AI_GATEWAY_PROVIDER_TIMEOUT_MS`, parsed and
range-validated at startup. An unparsable or non-positive value fails startup,
which the constitution's Configuration standard requires ("Invalid critical
configuration MUST cause the application to fail fast").

**Alternative considered**: *Per-adapter timeouts from provider metadata*.
Rejected; deferred to Phase 5, where a real provider may legitimately need a
longer deadline than the gateway default.

---

## D-005: Provider registration and resolution

**Decision**: A registry resolves the selected provider once at startup from
`AI_GATEWAY_PROVIDER`, defaulting to the deterministic provider. An unknown or
disabled name fails startup with a message naming the requested value.

**Rationale**: FR-005 and FR-008 require startup-time selection and
discoverability. The constitution's Configuration standard requires
distinguishing application configuration from provider configuration, so
provider selection is deliberately a separate variable from
`AI_GATEWAY_HOST`/`AI_GATEWAY_PORT` and is not folded into them. Failing fast on
an unknown name satisfies FR-007 and prevents the failure mode the constitution
warns about, where the process starts but every request fails.

**Discovery mechanism**: the selected provider name is reported through the
existing readiness surface and in the startup-refusal message. This adds no new
route, honouring the spec assumption that the gateway keeps serving exactly
three routes.

**Alternatives considered**:
- *Per-request provider selection via a request field*: rejected. It would
  change the client-facing request contract, which FR-015 freezes, and
  per-request routing is Phase 6.
- *A configuration file*: rejected by FR-028, which the user selected
  specifically to keep the existing environment-only pattern.

---

## D-006: How the boundary is proven with a second provider

**Decision**: The deterministic provider is the only provider shipped enabled.
A second provider is introduced as a **test double registered in tests only**,
and is unreachable from any shipped configuration.

**Rationale**: FR-006a was added during spec validation precisely because
resolving FR-026 left the specification inconsistent: SC-001, SC-002, SC-005,
FR-021, and User Story 3 all require proving the boundary against a second
provider, and with live providers excluded there would otherwise be nothing to
prove it against. Constitution principle IX requires provider interactions to
be "mockable or replaceable with deterministic test implementations" and
forbids requiring an external provider for ordinary tests, so a test-only
implementation is the constitution's own prescribed mechanism, not a
workaround.

**Consequence**: The multi-provider guarantees are verified in the test suite
rather than in a running deployment. This is stated plainly because it is a
real limitation of the phase, not a formality.

---

## D-007: Representing absent provider usage

**Decision**: The provider boundary reuses the existing domain type
`ChatResponse`, whose `usage` field is already `Option<Usage>`. The
client-facing response is **not changed**: it continues to carry exactly four
top-level keys (`id`, `object`, `model`, `choices`) and no `usage` key. Usage
becomes client-visible in Phase 13, not here.

**Rationale**: An earlier draft of this file treated absent usage as an open
conflict between FR-014 (represent absence) and FR-016 (stable response shape).
Checking the implementation showed the conflict does not exist. The Phase 2
response DTO has no `usage` field whatsoever, and
`chat_completion_response_serializes_exact_envelope_without_usage_or_created`
asserts that the serialized object has exactly four keys and contains neither
`usage` nor `created`. Since usage is not currently exposed to clients, a
provider that reports no usage cannot make the response shape
provider-dependent, and there is no new client-facing convention to invent.

This means:
- FR-014 is satisfied structurally. The gateway passes through exactly what a
  provider reported and has no code path that fabricates a count, because
  `ChatResponse::without_usage` already exists and is what the current mock
  uses.
- FR-016 is satisfied without qualification for the deterministic provider,
  which reports no usage today, and the response stays byte-identical to the
  current one.
- The domain type is reused unchanged, so no domain change is required for
  this feature.

**Deferred, with no ambiguity**: when Phase 13 adds usage to the client
response, it must decide how absence is rendered. That decision belongs with the
phase that introduces the field, and it is recorded there rather than decided
now on the strength of a hypothetical provider.

---

## D-008: Verifying nothing regresses

**Decision**: The existing 203-test suite must pass unchanged. New coverage is
additive, and the one existing behavioural test that touches completion
behaviour is used as the byte-identity anchor for FR-020 and FR-016.

**Rationale**: FR-015 and SC-006 require the Phase 3 request contract to be
untouched. The strongest available evidence is that the pre-existing suite,
which already asserts exact status codes, exact messages, exact body shapes,
and determinism across repeated requests, still passes. A regression here would
be visible as a failure rather than as an argument.

**Note on a known tension**: the existing mock service is synchronous and
infallible, returning a concrete type. Converting it to the async, fallible
provider contract is a real change to that code path. The constraint is that
its *observable* behaviour must not change, which the existing suite is
well-positioned to detect because it asserts exact response bodies today.

---

## Summary of new dependencies

| Dependency | Version | Purpose | Justification |
|---|---|---|---|
| `async-trait` | 0.1 | Allows the provider trait to be object-safe so providers can be held behind `Arc<dyn ...>` | D-002. Native `async fn` in traits was tested and is not dyn-compatible. |

No other dependency is added. No HTTP client is added, because FR-026 excludes
live provider connectivity; `reqwest` arrives in Phase 5 with the first
outbound call that needs it.
