# Runtime Configuration Contract: HTTP Gateway Core

**Feature**: [spec.md](../spec.md)

**Date**: 2026-09-25

## Purpose

Defines the process-local configuration required to bind and operate the Phase 2
HTTP gateway. This contract extends the `AI_GATEWAY_` namespace established by
the project foundation.

## Variables

| Variable | Type | Required | Default | Purpose |
|----------|------|----------|---------|---------|
| `AI_GATEWAY_HOST` | IPv4 or IPv6 literal | No | `127.0.0.1` | Listener host address |
| `AI_GATEWAY_PORT` | Integer in `1..=65535` | No | `3000` | Listener TCP port |

Example:

```bash
AI_GATEWAY_HOST=127.0.0.1 AI_GATEWAY_PORT=8080 cargo run --quiet
```

## Precedence

1. A present, valid environment variable overrides its default.
2. A present but empty or invalid value is an error; the application does not
   silently fall back to the default.
3. No `.env` file is loaded automatically. `.env.example` is documentation and a
   shell template only.

## Validation

- Host must parse as `std::net::IpAddr`.
- Port must parse as `u16` and must not be zero.
- The gateway validates configuration before reporting readiness.
- Binding occurs before the lifecycle changes from `Initializing` to `Ready`.
- Address-in-use, permission, and invalid-listener errors identify the attempted
  host and port in the startup diagnostic and return a non-zero exit status.

## Startup Sequence

```text
Read environment
    -> validate ServerConfig
    -> construct Initializing application state
    -> bind TcpListener
    -> build router
    -> transition to Ready
    -> serve HTTP
```

Any failure before `Ready` exits unsuccessfully and never produces a ready
result. The Axum accept loop is not driven while the state is `Initializing`;
router-level lifecycle tests still exercise the `Initializing` responses required
by SC-006.

## Shutdown

The Phase 2 in-flight allowance is fixed at 10 seconds. It is not configurable
through an environment variable in this phase.

## Secrets and Scope

These variables contain no credentials. Provider secrets, authentication
configuration, logging configuration, and external-service settings remain
outside this contract. Values shown in `.env.example` are placeholders only.

## Maintenance

Any new variable MUST update this contract, `.env.example`, configuration tests,
and user documentation in the same feature.
