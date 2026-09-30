# Quickstart: Provider Abstraction Validation Guide

**Feature**: `specs/005-provider-abstraction`
**Date**: 2026-09-30

Purpose: a runnable guide for verifying this feature end-to-end. It states
what to run and what to observe, not how to implement it. Implementation
detail belongs in `tasks.md`.

Read alongside [spec.md](spec.md), [data-model.md](data-model.md), and
[contracts/provider-contract.md](contracts/provider-contract.md).

---

## Prerequisites

| Requirement | Notes |
|---|---|
| Rust 1.98.1 | Pinned in `rust-toolchain.toml`; the toolchain file selects it automatically |
| No network access | The default configuration performs zero egress |
| No credentials | The default provider needs none |
| No configuration | The gateway runs with zero environment variables set |

---

## 1. Quality gates

All six must pass. This is the same gate every prior phase used.

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo check --all-targets
cargo test --all-targets
cargo build
cargo build --release
```

**Expected**: every command succeeds. Any failure blocks the feature.

---

## 2. Baseline: the suite must not shrink or regress

Phase 3 ended at 203 tests. This feature adds coverage and MUST NOT remove or
weaken any existing test.

```bash
cargo test --all-targets 2>&1 | grep -E '^test result'
```

**Expected**:

- The library, `http_api`, and `server_lifecycle` suites all report
  `0 failed`.
- The total is **at least 203**, and higher once this feature's tests land.
- No previously passing test is renamed, deleted, or weakened.

**If a test was changed rather than added**, that is a red flag. The only
legitimate edits to existing tests are those forced by a deliberate contract
change, and the only such change here is the addition of two error codes, which
is additive and should not require editing any Phase 3 test.

---

## 3. Default startup: zero configuration, zero egress

Confirms the gateway still runs with nothing set, which is what makes the
abstraction safe to land before any real provider exists.

```bash
env -u AI_GATEWAY_PROVIDER -u AI_GATEWAY_PROVIDER_TIMEOUT_MS \
  cargo run --quiet
```

In a second shell:

```bash
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3000/health
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3000/ready
curl -s -X POST http://127.0.0.1:3000/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"general","messages":[{"role":"user","content":"Hello"}]}'
```

**Expected**:

- `/health` → `200`
- `/ready` → `200`
- chat → `200` with a body containing exactly the four keys `id`, `object`,
  `model`, `choices`, and **no** `usage` and **no** `created` key.
- No outbound connection is attempted. The process must run with networking
  unavailable.

Stop with `Ctrl-C` and confirm a clean shutdown message.

---

## 4. Provider selection

### 4.1 The default provider is selected implicitly

Start with no provider variable, as in section 3.

**Expected**: the gateway starts, serves traffic, and its readiness output
names the selected provider. Selection is discoverable without reading source
(FR-008).

### 4.2 An unknown provider fails startup

```bash
AI_GATEWAY_PROVIDER=does-not-exist cargo run --quiet
```

**Expected**: the process **refuses to start** and the message names
`does-not-exist`. It MUST NOT start and then fail every request (FR-007).

### 4.3 A disabled provider fails startup

```bash
AI_GATEWAY_PROVIDER=<a known-but-disabled-provider-id> cargo run --quiet
```

**Expected**: startup is refused and the message names the requested value.
No second provider ships enabled, so this exercises the disabled path.

### 4.4 An invalid deadline fails startup

```bash
AI_GATEWAY_PROVIDER_TIMEOUT_MS=not-a-number cargo run --quiet
```

```bash
AI_GATEWAY_PROVIDER_TIMEOUT_MS=0 cargo run --quiet
```

**Expected**: both refuse to start and name the variable. A non-positive
deadline is rejected rather than silently treated as "no timeout".

---

## 5. The client-facing error contract

### 5.1 Phase 3 rows are unchanged

Send each of these and confirm the status, code, and message are byte-identical
to Phase 3:

| Request | Expected |
|---|---|
| `{"model":"","messages":[{"role":"user","content":"Hi"}]}` | `400` `invalid_request` |
| `{"model":"general","messages":[]}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"nope","content":"Hi"}]}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"temperature":9.0}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"max_tokens":0}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"max_tokens":"8"}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"temperature":null}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"temperature":0.5,"temperature":1.0}` | `400` `invalid_request` |
| `{"model":"general","messages":[{"role":"user","content":"Hi"}],"stream":true}` | `400` `unsupported_feature` |
| `not json` | `400` `invalid_request` |

**Expected**: every one returns the same code and message as it did in Phase 3.
`temperature: 2.0` and `max_tokens: 4096` remain accepted, and
`max_tokens: 4095` remains refused — the inclusive boundaries are unchanged.

### 5.2 Every error body has exactly two keys

```bash
curl -s -X POST http://127.0.0.1:3000/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"","messages":[]}' | python3 -m json.tool
```

**Expected**: `{"code": ..., "message": ...}` and nothing else. No `details`,
no wrapper, no per-field list, and no field name from the request.

---

## 6. The new provider-failure rows

These require a provider that fails. Because live connectivity is out of scope,
they are driven by the test double through the suite rather than by a running
server.

```bash
cargo test --all-targets provider_failure 2>&1 | grep -E '^test |^test result'
```

**Expected**: coverage for all five failure categories, asserting:

| Category | Status | Code | Message |
|---|---|---|---|
| `Unreachable` | `502` | `provider_unavailable` | `The upstream provider is unavailable.` |
| `Refused` | `502` | `provider_unavailable` | `The upstream provider is unavailable.` |
| `DeadlineExceeded` | `504` | `provider_timeout` | `The upstream provider did not respond in time.` |
| `UnusableResponse` | `500` | `internal_error` | `The gateway could not complete the request. |
| `InvalidResponse` | `500` | `internal_error` | `The gateway could not complete the request. |

---

## 7. No provider detail leaks

The most important property in this feature, because it is the one that is
easy to break later when a real provider is added.

```bash
cargo test --all-targets leak 2>&1 | grep -E '^test result'
```

**Expected**: tests assert that, for every failure category, the response body
equals the fixed string **exactly**. Provider status codes, provider message
text, provider URLs, provider model names, and credentials must appear nowhere.
A body that merely *contains* the code is not sufficient; it must be equal.

---

## 8. Boundary invariants

Each maps to an invariant in
[contracts/provider-contract.md](contracts/provider-contract.md).

```bash
cargo test --all-targets -- provider isolation concurrent determin 2>&1 \
  | grep -E '^test result'
```

**Expected**:

- **Determinism**: identical input to the deterministic provider yields a
  byte-identical response, repeatedly.
- **Isolation**: concurrent requests to different providers each receive their
  own provider's response, with no cross-contamination and no request served
  by the wrong provider.
- **No provider call before validation**: an invalid request and an oversized
  request are both refused without any provider being contacted.
- **Containment**: after an induced provider failure, `/health` and `/ready`
  still return `200` and a subsequent valid request still succeeds.
- **Graceful shutdown**: a request in flight when shutdown begins is allowed
  to finish or is abandoned deliberately, and no new provider call is accepted
  afterwards.

---

## 9. Adding a provider touches one file

This is the durability test of the abstraction (FR-021, SC-001). It is a
developer exercise rather than a command, and its result is recorded in
`tasks.md` when the feature is implemented.

1. Implement `LlmProvider` in a single new file under the infrastructure layer.
2. Register it.
3. Run the suite.

**Expected**: the full suite passes with no edit to request validation, to the
client-facing response, or to any other provider. Then remove the file and
confirm the suite passes again unchanged.

If any step required touching an unrelated file, the boundary has leaked and
the feature is not complete.

---

## 10. Contract artifacts agree

```bash
grep -c '^| ' specs/005-provider-abstraction/contracts/http-api.md
```

**Expected**: the client-facing error table in
[contracts/http-api.md](contracts/http-api.md) lists **10** codes, matching the
implementation. Cross-check that `docs/api.md` was updated to the same count,
since Phase 3 recorded eight there and left 502 and 504 listed as planned.

---

## Completion criteria

This feature is verified when all of the following hold:

1. All six quality gates pass.
2. The test total is at least 203 with zero failures and no weakened test.
3. The gateway runs with no configuration and no network.
4. An unknown provider, a disabled provider, and an invalid deadline each
   refuse startup and name the offending value.
5. Every Phase 3 error row is byte-identical to Phase 3.
6. Every error body has exactly two keys.
7. All five provider failure categories map to the documented status and code.
8. No provider detail reaches a client.
9. Adding a provider requires editing only that provider.
10. `docs/api.md` and the contracts agree on ten error codes.
