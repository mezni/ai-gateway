# Quickstart / Validation Guide: HTTP Gateway Core

**Feature**: [spec.md](spec.md) | **Contract**: [HTTP API](contracts/http-api.md) | **Configuration**: [runtime](contracts/runtime-configuration.md)

This guide validates the local Phase 2 gateway without credentials, providers, or
other external services.

## Prerequisites

- Rust 1.98.1, installed through `rust-toolchain.toml`.
- `curl` for HTTP requests.
- A checkout containing the completed Phase 2 implementation.

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

**Expected**: every command succeeds. The test suite includes endpoint,
validation, concurrency, readiness, and bound-port shutdown coverage.

## Start the Gateway

Build first, then run the binary directly so the recorded PID is the gateway:

```bash
BASE_URL=http://127.0.0.1:3000
./target/debug/ai-gateway > /tmp/ai-gateway-http-core.log 2>&1 &
GATEWAY_PID=$!
trap 'kill "$GATEWAY_PID" 2>/dev/null || true' EXIT

READY=0
for attempt in $(seq 1 50); do
  if curl --silent --fail "${BASE_URL}/health" >/dev/null; then
    READY=1
    break
  fi
  sleep 0.1
done
test "$READY" -eq 1
```

**Expected**: the process remains running and binds `127.0.0.1:3000` by default.

## Scenario 1: Liveness

```bash
curl --include --silent "${BASE_URL}"/health
```

**Expected**:

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{"status":"ok"}
```

## Scenario 2: Readiness

```bash
curl --include --silent "${BASE_URL}"/ready
```

**Expected**:

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{"status":"ready"}
```

## Scenario 3: Deterministic Mock Completion

```bash
curl --include --silent \
  --header 'Content-Type: application/json' \
  --data '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}' \
  "${BASE_URL}"/v1/chat/completions
```

**Expected**: HTTP 200, `Content-Type: application/json`, and:

```json
{
  "id": "chat_mock",
  "object": "chat.completion",
  "model": "mock-model",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "This is a mocked chat completion."
      },
      "finish_reason": "stop"
    }
  ]
}
```

Repeat the request and compare parsed JSON. The complete response is identical
and contains no `usage` field.

## Scenario 4: Invalid Requests

```bash
curl --include --silent \
  --header 'Content-Type: application/json' \
  --data '{"messages":[]}' \
  "${BASE_URL}"/v1/chat/completions
```

**Expected**: HTTP 400 and exactly these error fields:

```json
{
  "code": "invalid_request",
  "message": "The chat request is invalid."
}
```

Also verify malformed JSON, a non-object body, a missing model, an empty message
list, an unknown role, and empty content return the same status, code, and safe
message without echoing the prompt or parser diagnostics.

## Scenario 5: Streaming Rejection

```bash
curl --include --silent \
  --header 'Content-Type: application/json' \
  --data '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"stream":true}' \
  "${BASE_URL}"/v1/chat/completions
```

**Expected**: HTTP 400 with:

```json
{
  "code": "unsupported_feature",
  "message": "Streaming is not supported."
}
```

## Scenario 6: Unsupported Media Type

```bash
curl --include --silent \
  --header 'Content-Type: text/plain' \
  --data '{"model":"mock-model","messages":[]}' \
  "${BASE_URL}"/v1/chat/completions
```

**Expected**: HTTP 415 with code `unsupported_media_type` and the exact message
from the HTTP contract.

## Scenario 7: Method and Unknown Path

```bash
curl --include --silent --request POST "${BASE_URL}"/health
curl --include --silent --head "${BASE_URL}"/health
curl --include --silent "${BASE_URL}"/not-a-route
```

**Expected**: the POST and HEAD requests both return HTTP 405 with `Allow: GET`
(the HEAD response has no body, as required by HTTP), and the unknown path
returns HTTP 404. Error fields match the contract.

## Scenario 8: Runtime Configuration

```bash
AI_GATEWAY_HOST=127.0.0.1 AI_GATEWAY_PORT=0 ./target/debug/ai-gateway
```

**Expected**: non-zero exit and a clear invalid-port diagnostic; the application
does not report ready.

Stop the first gateway before testing a valid alternate port:

```bash
kill -TERM "$GATEWAY_PID"
wait "$GATEWAY_PID"
BASE_URL=http://127.0.0.1:3001
AI_GATEWAY_PORT=3001 ./target/debug/ai-gateway > /tmp/ai-gateway-http-core.log 2>&1 &
GATEWAY_PID=$!

READY=0
for attempt in $(seq 1 50); do
  if curl --silent --fail "${BASE_URL}/ready" >/dev/null; then
    READY=1
    break
  fi
  sleep 0.1
done
test "$READY" -eq 1

curl --silent --fail "${BASE_URL}/ready"
```

**Expected**: the second process reports `{"status":"ready"}` on port 3001.

## Scenario 9: Clean Shutdown

```bash
kill -TERM "$GATEWAY_PID"
wait "$GATEWAY_PID"
GATEWAY_PID=
```

**Expected**: the process exits successfully within 10 seconds and releases the
listening socket. Automated lifecycle tests additionally hold an admitted request
open to verify that health remains `ok`, readiness becomes `not_ready`, new chat
requests receive 503, and the bound server closes after drain.

## Scenario 10: Local Performance Checks

Restart the gateway on port 3001 for the remaining checks:

```bash
BASE_URL=http://127.0.0.1:3001
AI_GATEWAY_PORT=3001 ./target/debug/ai-gateway > /tmp/ai-gateway-http-core.log 2>&1 &
GATEWAY_PID=$!

READY=0
for attempt in $(seq 1 50); do
  if curl --silent --fail "${BASE_URL}/health" >/dev/null; then
    READY=1
    break
  fi
  sleep 0.1
done
test "$READY" -eq 1

for request in $(seq 1 100); do
  curl --silent --output /dev/null \
    --write-out '%{http_code} %{time_total}\n' \
    "${BASE_URL}/health"
done

for request in $(seq 1 100); do
  curl --silent --output /dev/null \
    --write-out '%{http_code} %{time_total}\n' \
    "${BASE_URL}/ready"
done
```

**Expected**: all status codes are 200 and at least 95 of each 100 requests
complete within 0.250 seconds on an otherwise idle local machine.

## Scenario 11: Offline Operation

Run the endpoint scenarios with no provider environment variables, network
service, database, cache, or credential store configured.

**Expected**: all three endpoints work. No provider or external service is
contacted.

## Cleanup

If a scenario failed before normal shutdown:

```bash
kill -TERM "$GATEWAY_PID" 2>/dev/null || true
wait "$GATEWAY_PID" 2>/dev/null || true
```

## References

- HTTP contract: [contracts/http-api.md](contracts/http-api.md)
- Configuration contract: [contracts/runtime-configuration.md](contracts/runtime-configuration.md)
- Data model: [data-model.md](data-model.md)
- Requirements: [spec.md](spec.md)
