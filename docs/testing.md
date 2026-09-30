# Testing Strategy

## 1. Overview

This gateway has no external dependency at runtime, so every test runs offline.
There is no provider, database, cache, or network call in the request path, and
the test suite proves that rather than assuming it.

Three test binaries cover the service:

| Binary | Location | Scope |
|--------|----------|-------|
| Library unit tests | `src/**` inline `mod tests` | Domain rules, validation pipeline, DTO parsing, error mapping, middleware |
| HTTP contract tests | `tests/http_api.rs` | Router-level request and response behavior |
| Server lifecycle tests | `tests/server_lifecycle.rs` | A real bound socket, shutdown, and in-flight draining |

## 2. Commands

Run everything:

```bash
cargo test --all-targets
```

Run one binary or one module:

```bash
cargo test --lib                      # library unit tests only
cargo test --test http_api            # HTTP contract tests only
cargo test --test server_lifecycle    # bound-server lifecycle tests only
cargo test --lib application::chat    # one module by path
cargo test --test http_api chat_rejects_a_repeated_generation_control
```

The full quality gate, which must pass before a change is considered done:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo check --all-targets
cargo test --all-targets
cargo build
cargo build --release
```

## 3. Test Groups

### 3.1 Rule Table

`every_documented_rule_produces_its_exact_response` in `tests/http_api.rs` is
table-driven: it carries one violating request per rule identifier from
`specs/004-domain-validation/contracts/validation-rules.md` §2, all 20 of them,
each paired with the exact status, code, and message §4 requires.

`the_rule_suite_covers_every_catalogued_rule_id` writes the expected 20
identifiers out in full and compares them to the set the suite exercises. Adding
a rule to the contract without adding a case therefore fails the build, which is
how the catalog is kept from drifting away from the tests.

`every_failure_response_has_exactly_two_fields_and_no_details` re-runs the whole
table and asserts each body is exactly `code` and `message`, with no `details`,
`error`, `source`, `diagnostic`, or `request_id` key.

### 3.2 Precedence

`precedence_resolves_combined_violations_to_the_earliest_stage` breaks every
stage at or before the one it names, covering all nine stages of the precedence
order. Each case is repeated 20 times to prove the outcome is stable rather than
incidental.

`a_request_breaking_model_message_control_and_size_rules_at_once_reports_size`
sends one body that violates the model, message, control, and size rules
simultaneously and asserts the earliest stage wins.

### 3.3 Size Boundary

The 1 MiB limit is inclusive, so the tests hit the boundary from both sides:

- `body_of_exactly_one_mebibyte_is_accepted_and_one_byte_more_is_refused` —
  1 048 576 bytes returns 200, 1 048 577 returns 413.
- `MAX_REQUEST_BODY_BYTES` is asserted to equal `1_048_576` in
  `src/api/middleware.rs`.
- Refusal is proven to precede the media type check, so an oversized
  `text/plain` body returns 413 and not 415.
- A body with no declared `Content-Length`, and one with an unparsable
  `Content-Length`, are both still refused.
- `an_oversized_refusal_still_releases_the_admission_slot` verifies through
  `lifecycle.wait_for_zero()` that a refused oversized request cannot stall
  graceful shutdown.

### 3.4 Control Ranges

`chat_rejects_every_malformed_or_out_of_range_control` covers every control
failure in the rule catalog, and `chat_rejects_a_repeated_generation_control`
covers duplicates, which need raw JSON text because `json!` keeps only the last
copy of a repeated key.

`control_failure_response_leaks_no_value_field_name_or_parser_detail` compares
the raw response bytes and asserts the submitted value, a prompt marker, the
field name, and parser text are all absent.

The equivalent checks exist at the DTO layer in `src/api/dto.rs` for
non-numeric `temperature`, non-integer `max_tokens`, and explicit `null`.

### 3.5 Concurrency And Determinism

- `fifty_simultaneous_mixed_requests_stay_isolated` — 50 requests released
  together by a barrier, every fifth one invalid, each response checked against
  the model that request sent. This is what proves isolation rather than merely
  absence of failure.
- `the_canonical_request_is_deterministic_with_and_without_controls` — 200
  alternating repetitions compared byte for byte.

### 3.6 Lifecycle

- `health_and_readiness_hold_before_during_and_after_every_request_kind` probes
  `/health` and `/ready` before, between, and after valid, invalid, and oversized
  requests.
- `a_long_run_of_oversized_requests_does_not_degrade_later_handling` runs 10
  oversized refusals, then asserts the next real request is served exactly as the
  first ever request would be.
- `admission_precedes_media_type_and_body_handling` and
  `oversized_body_is_still_refused_while_the_gateway_is_not_ready` pin the stage
  ordering.

### 3.7 Contract Drift

`every_contract_row_is_covered_and_byte_exact` holds all 13 wire-reachable rows
of the Client-Facing Validation Contract table in
`specs/004-domain-validation/spec.md` and compares the whole response body
byte-for-byte against the expected `{"code":...,"message":...}`.

`the_contract_table_has_exactly_fourteen_rows` accounts for the 14th row,
`internal_error`, which no request in the contract can trigger and which is
therefore asserted at the error-mapping layer.

### 3.8 No External Dependency

`validation_reaches_no_external_resource` drives valid, invalid, and refused
requests with no provider, credential, database, cache, or network
configuration present, and asserts that `/health` and `/ready` still report
truthfully afterwards.

## 4. Conventions

- A test name states the observable behavior, not the function under test:
  `body_of_exactly_one_mebibyte_is_accepted_and_one_byte_more_is_refused`.
- Boundary tests hit the boundary from both sides. An inclusive limit is only
  proven by testing the last accepted value and the first refused one.
- Error tests assert the exact status, code, and message, and usually the raw
  bytes, so an extra key or substituted text fails.
- Leak tests name concrete strings to keep out of a response, including a
  prompt marker, rather than asserting a vague absence.
- When a test fails, decide whether the code or the test is wrong before
  changing either, and record any intentional behavior change. During the
  domain-validation feature, one pre-existing test asserted that
  `temperature: -10.0` and `max_tokens: -1` were accepted; that encoded exactly
  the behavior the feature replaced, so the test was rewritten to check its real
  intent — that unknown fields are ignored.
- The suite must never lose a test. The baseline recorded before the feature
  began is 120 tests: 87 library, 28 `http_api`, 5 `server_lifecycle`.

## 5. Writing A New Test

1. Put the test where its subject lives: a rule of the domain goes in
   `src/domain/chat.rs`, a parsing or mapping decision in the owning `src/api`
   module, and wire behavior in `tests/http_api.rs`.
2. Confirm the test fails before the behavior exists.
3. Assert the exact contract, not a weaker proxy. For an error, assert status,
   code, and message, and assert nothing else is present.
4. Run `cargo test --all-targets`, then the full quality gate in Section 2.
