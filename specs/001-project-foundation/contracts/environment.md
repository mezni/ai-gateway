# Environment Variable Contract

**Feature**: [Project Foundation](../spec.md)
**Date**: 2026-09-24

## Purpose

This contract defines how the gateway reads configuration from its
environment. It governs `.env.example` and every future environment variable
added by later phases.

## Conventions

- Names use `SCREAMING_SNAKE_CASE`.
- All gateway-owned variables live under the `AI_GATEWAY_` namespace.
- Provider/secret variables (added by later phases) use their provider's
  conventional names (e.g., `OPENROUTER_API_KEY`).
- `.env.example` contains placeholder values only — never real values.
- Each variable entry documents: name, required/optional, default (if any),
  and purpose.
- Values are read directly from the process environment at startup; the
  gateway adds no dotenv loader and no other configuration source.

## Current Read-Set

The application reads exactly two environment variables. Both are optional,
both live in the `AI_GATEWAY_` namespace, and both are read at startup by
`ServerConfig::from_env()` in `src/config/server.rs`.

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `AI_GATEWAY_HOST` | No | `127.0.0.1` | Listener host address |
| `AI_GATEWAY_PORT` | No | `3000` | Listener TCP port |

### `AI_GATEWAY_HOST`

- **Required**: No; the default applies when the variable is unset.
- **Default**: `127.0.0.1`.
- **Purpose**: Selects the address the HTTP listener binds to.
- **Validation**: Must parse as an IPv4 or IPv6 literal. An empty or malformed
  value is a startup failure.

### `AI_GATEWAY_PORT`

- **Required**: No; the default applies when the variable is unset.
- **Default**: `3000`.
- **Purpose**: Selects the TCP port the HTTP listener binds to.
- **Validation**: Must parse as a non-zero `u16`; port `0` is rejected. An empty
  or malformed value is a startup failure.

A present but empty or malformed value never falls back to the default. The
process exits non-zero with a contextual error such as
`invalid gateway configuration`. Precedence and startup sequencing for these
variables is owned by the HTTP Gateway Core runtime configuration contract
(`specs/003-http-gateway-core/contracts/runtime-configuration.md`).

## Template (`.env.example`)

```bash
# AI Gateway environment template.
# Copy to .env for local development: cp .env.example .env
# Values are placeholders only — never commit real secrets.

# Non-secret gateway configuration defaults.
AI_GATEWAY_HOST=127.0.0.1
AI_GATEWAY_PORT=3000

# No .env file is loaded automatically; export these values before starting.
```

## Rules

1. Every variable the application reads MUST be documented in `.env.example`
   (FR-007).
2. `.env.example` MUST only ever contain placeholder values (FR-007, FR-012).
3. `.env` MUST remain untracked by version control (FR-008).
4. `.env` is never read at runtime. Copying `.env.example` to `.env` has no
   effect on the process, because no dotenv loader is linked.
5. Variables MUST be exported in the shell, or set inline for a single run,
   before the application starts (for example
   `AI_GATEWAY_PORT=3001 cargo run --quiet`).
6. An absent variable applies its documented default; a present but empty or
   malformed value MUST fail startup rather than fall back to the default.
7. Later phases that introduce variables MUST update this contract before
   implementation.

## Maintenance

This contract is authoritative for configuration naming and documentation.
Changes MUST be reviewed against [privacy & secrets](../spec.md) requirements
(FR-012, SC-004).
