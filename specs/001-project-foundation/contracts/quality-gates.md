# Quality Gate Contract

**Feature**: [Project Foundation](../spec.md)
**Date**: 2026-09-24

## Purpose

Defines the development workflow checks that MUST pass before any change is
accepted into the AI Gateway repository. These are the definition-of-done
gates for foundation and every later phase.

## Gates

Run in this order from a clean checkout:

| # | Check | Command | Pass condition |
|---|-------|---------|----------------|
| 1 | Format | `cargo fmt --all --check` | No diff reported |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` | Zero warnings/errors |
| 3 | Compile | `cargo check --all-targets` | Succeeds |
| 4 | Tests | `cargo test --all-targets` | All tests pass |
| 5 | Debug build | `cargo build` | Succeeds |
| 6 | Release build | `cargo build --release` | Succeeds |
| 7 | Smoke run | `cargo run --quiet` | Service responds, then exits 0 |

## Smoke Run

The service is long-running, so `cargo run --quiet` starts a foreground HTTP
server and does not exit on its own. The smoke step verifies that it serves
traffic and then terminates with exit 0 on `SIGTERM`.

```bash
AI_GATEWAY_PORT=3001 cargo run --quiet &
GATEWAY_PID=$!

BASE_URL=http://127.0.0.1:3001
for attempt in $(seq 1 50); do
  curl --silent --fail "${BASE_URL}/ready" >/dev/null && break
  sleep 0.1
done

curl --silent --fail "${BASE_URL}/ready"   # {"status":"ready"}
curl --silent --fail "${BASE_URL}/health"  # {"status":"ok"}
curl --silent --fail -X POST "${BASE_URL}/v1/chat/completions" \
  -H 'Content-Type: application/json' \
  -d '{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}'

kill -TERM "$GATEWAY_PID"
wait "$GATEWAY_PID"   # exit status 0
```

This step is a manual local check performed by the contributor. The
`cargo test --all-targets` gate already covers the lifecycle and shutdown
behavior automatically, including the real-socket startup and `SIGTERM`
drain tests, so a failure here signals an environment or build problem rather
than a gap in automated coverage.

## Failure Behavior

- A failing gate MUST block the change from merging.
- Check output MUST make the failure identifiable (file/line or test name).
- There is no manual bypass; a contributor fixes the violation and re-runs
  the full gate sequence.

## Scope Notes

- `cargo audit`/`cargo deny` dependency scanning remains a CI/CD-phase task,
  even though the project now has dependencies.
- CI automation is a CI/CD-phase task; these commands here define the local
  workflow contract that CI will mirror.

## Verification (maps to success criteria)

- SC-002: gates pass consistently on repeated clean-environment runs.
- SC-005: the project remains runnable after every change (gates enforced
  from the first commit).
