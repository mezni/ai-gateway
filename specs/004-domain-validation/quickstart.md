# Quickstart / Validation Guide: Domain Model Validation

**Feature**: [spec.md](spec.md) | **Rules**: [validation-rules.md](contracts/validation-rules.md) | **Wire contract**: [contracts/http-api.md](contracts/http-api.md) | **Model**: [data-model.md](data-model.md)

This guide validates the Phase 3 gateway without credentials, providers, or any
other external service. Every scenario is a request/response check or a command
run; the phase's rule logic is additionally covered by the automated suite.

## Prerequisites

- Rust 1.98.1, installed through `rust-toolchain.toml`.
- `curl` and `python3` for building exact-size request bodies.
- A checkout containing the completed Phase 3 implementation.

## Setup and Quality Gates

From the repository root:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo check --all-targets
cargo test --all-targets
cargo build
cargo build --release
```

**Expected**: every command succeeds. The test suite includes the existing
endpoint, validation, concurrency, readiness, and shutdown coverage plus the
rule-by-rule, boundary, and precedence checks for this phase.

## Start the Gateway

```bash
BASE_URL=http://127.0.0.1:3000
./target/debug/ai-gateway > /tmp/ai-gateway-domain-validation.log 2>&1 &
GATEWAY_PID=$!
trap 'kill "$GATEWAY_PID" 2>/dev/null || true' EXIT

READY=0
for attempt in $(seq 1 50); do
  if curl --silent --fail "${BASE_URL}/ready" >/dev/null; then
    READY=1
    break
  fi
  sleep 0.1
done
test "$READY" -eq 1
```

**Expected**: the process remains running and reports ready.

Define a helper that prints only the status, code, and message of a response so
each scenario can be compared at a glance:

```bash
post() {
  curl --silent --output /tmp/ai-gateway-body.json \
    --write-out '%{http_code}\n' \
    --header 'Content-Type: application/json' \
    --data "$1" "${BASE_URL}/v1/chat/completions"
  cat /tmp/ai-gateway-body.json; echo
}
```

## Scenario 1: Canonical Request

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}'
```

**Expected**: HTTP 200 and the mock completion envelope from the wire contract,
with no `usage` field.

## Scenario 2: In-Range Generation Controls

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.7,"max_tokens":500}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.0,"max_tokens":1}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":2.0,"max_tokens":4096}'
```

**Expected**: three HTTP 200 responses whose bodies are identical to Scenario 1.
Accepting a control does not change the mock output, because no model is
connected.

## Scenario 3: Controls Omitted

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}'
```

**Expected**: HTTP 200, byte-identical to Scenario 2. The gateway does not invent
a control value and does not report one as client-supplied.

## Scenario 4: Out-of-Range Controls

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":2.0001}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":-0.1}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":4097}'
post '{"model":"mock-template","messages":[{"role":"user","content":"Hello"}],"max_tokens":0}'
```

**Expected**: four HTTP 400 responses, each exactly:

```json
{
  "code": "invalid_request",
  "message": "The chat request is invalid."
}
```

The `0.0`/`2.0` and `1`/`4096` boundaries were accepted in Scenario 2; these
are the first values outside them.

## Scenario 5: Non-Numeric Controls

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":"0.5"}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":true}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":null}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":[500]}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":100.5}'
```

**Expected**: five HTTP 400 `invalid_request` responses and no 500. A `null`
control is a refusal, not an omission; `100.5` is refused because a token count
is an integer.

## Scenario 6: Repeated Control

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":10,"max_tokens":20}'
```

**Expected**: two HTTP 400 `invalid_request` responses. Neither occurrence is
silently selected.

## Scenario 7: Required Fields, Roles, and Content

```bash
post '{"messages":[{"role":"user","content":"Hello"}]}'
post '{"model":"mock-model","messages":[]}'
post '{"model":"mock-model","messages":[{"role":"tool","content":"Hello"}]}'
post '{"model":"mock-model","messages":[{"role":"user","content":""}]}'
post '{"model":"","messages":[{"role":"user","content":"Hello"}]}'
```

**Expected**: five HTTP 400 `invalid_request` responses with the fixed message
and no echo of the submitted model, role, or content.

## Scenario 8: No Diagnostic Leakage

Submit a rejected request whose values are unique markers, then confirm none of
them appear in the response:

```bash
body='{"model":"LEAK_MODEL_MARKER","messages":[{"role":"LEAK_ROLE_MARKER","content":"LEAK_PROMPT_MARKER"}]}'
curl --silent --header 'Content-Type: application/json' --data "$body" \
  "${BASE_URL}/v1/chat/completions" > /tmp/ai-gateway-leak.json
for marker in LEAK_MODEL_MARKER LEAK_ROLE_MARKER LEAK_PROMPT_MARKER missing field invalid type serde; do
  if grep --quiet --fixed-strings "$marker" /tmp/ai-gateway-leak.json; then
    echo "LEAKED: $marker"; exit 1
  fi
done
cat /tmp/ai-gateway-leak.json
```

**Expected**: no `LEAKED` line; the body is exactly the two-field
`invalid_request` object.

## Scenario 9: Request Size Boundary

`SIZE_LIMIT` is 1 048 576 bytes. Build a valid request padded with a long
content string to hit each size exactly:

```bash
build_body() {
  python3 - "$1" <<'PY'
import json, sys
target = int(sys.argv[1])
base = {"model": "mock-model", "messages": [{"role": "user", "content": ""}]}
pad = target - len(json.dumps(base, separators=(",", ":")))
base["messages"][0]["content"] = "x" * max(pad, 0)
body = json.dumps(base, separators=(",", ":"))
assert len(body) == target, (len(body), target)
print(body)
PY
}

AT_LIMIT=$(build_body 1048576)
OVER_LIMIT=$(build_body 1048577)
printf '%s' "${AT_LIMIT}" | wc -c
printf '%s' "${OVER_LIMIT}" | wc -c

curl --silent --output /dev/null --write-out 'at limit:   %{http_code}\n' \
  --header 'Content-Type: application/json' --data-binary "${AT_LIMIT}" \
  "${BASE_URL}/v1/chat/completions"
curl --silent --write-out 'over limit: %{http_code}\n' \
  --header 'Content-Type: application/json' --data-binary "${OVER_LIMIT}" \
  "${BASE_URL}/v1/chat/completions"
```

**Expected**: the two byte counts are `1048576` and `1048577`; the responses are
`at limit: 200` and `over limit: 413`, the latter with exactly:

```json
{
  "code": "payload_too_large",
  "message": "The request payload is too large."
}
```

Then confirm the gateway is unharmed:

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}'
curl --silent --output /dev/null --write-out 'health: %{http_code}\n' "${BASE_URL}/health"
curl --silent --output /dev/null --write-out 'ready:  %{http_code}\n' "${BASE_URL}/ready"
```

**Expected**: the canonical request returns 200 and the probes return 200.

## Scenario 10: Precedence

Oversize wins over media type, because an oversized body cannot be validated
without reading it:

```bash
curl --silent --output /dev/null --write-out 'oversize + text/plain: %{http_code}\n' \
  --header 'Content-Type: text/plain' --data-binary "${OVER_LIMIT}" \
  "${BASE_URL}/v1/chat/completions"
```

**Expected**: `413`, not `415`.

Invalidity wins over streaming:

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"stream":true}'
post '{"model":"","messages":[],"stream":true}'
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":9.0,"stream":true}'
```

**Expected**: `400 unsupported_feature` for the first and
`400 invalid_request` for the second and third.

## Scenario 11: Streaming, Media Type, Method, and Path

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"stream":true}'
curl --silent --write-out '\n' --header 'Content-Type: text/plain' \
  --data '{"model":"mock-model","messages":[]}' "${BASE_URL}/v1/chat/completions"
curl --silent --output /dev/null --write-out 'POST /health: %{http_code}\n' --request POST "${BASE_URL}/health"
curl --silent --output /dev/null --write-out '/not-a-route: %{http_code}\n' "${BASE_URL}/not-a-route"
```

**Expected**: `400 unsupported_feature`; `415 unsupported_media_type`;
`405 method_not_allowed`; `404 not_found`. These rows are unchanged from Phase 2.

## Scenario 12: Unknown Fields Are Ignored

```bash
post '{"model":"mock-model","messages":[{"role":"user","content":"Hello","name":"ignored"}],"top_p":0.1,"future_option":{"a":1}}'
```

**Expected**: HTTP 200 with the mock completion.

## Scenario 13: Determinism and Concurrency

```bash
for request in $(seq 1 100); do
  post '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}' >> /tmp/ai-gateway-repeat.txt
done
sort --unique /tmp/ai-gateway-repeat.txt | wc -l
```

**Expected**: `1` — every repetition returns the same status and the same body.

```bash
for request in $(seq 1 50); do
  curl --silent --output /dev/null --write-out '%{http_code}\n' \
    --header 'Content-Type: application/json' \
    --data '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":1.5,"max_tokens":100}' \
    "${BASE_URL}/v1/chat/completions" &
done
wait
```

**Expected**: 50 lines of `200`, with no cross-request interference. Automated
tests additionally mix valid and invalid requests concurrently.

## Scenario 14: Offline Operation

Run the scenarios with no provider environment variable, credential, database,
cache, or other service configured, and with no network access beyond loopback.

**Expected**: every scenario behaves exactly as above. Validation contacts no
external service.

## Scenario 15: Clean Shutdown

```bash
kill -TERM "$GATEWAY_PID"
wait "$GATEWAY_PID"
GATEWAY_PID=
```

**Expected**: the process exits successfully within 10 seconds and releases the
socket.

## Cleanup

If a scenario failed before normal shutdown:

```bash
kill -TERM "$GATEWAY_PID" 2>/dev/null || true
wait "$GATEWAY_PID" 2>/dev/null || true
```

## References

- Rule catalog: [contracts/validation-rules.md](contracts/validation-rules.md)
- Wire contract: [contracts/http-api.md](contracts/http-api.md)
- Data model: [data-model.md](data-model.md)
- Design decisions: [research.md](research.md)
- Requirements: [spec.md](spec.md)
