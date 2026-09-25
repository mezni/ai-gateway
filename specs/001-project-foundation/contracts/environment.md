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

## Current Read-Set

In the foundation phase the application reads **no** environment variables.
`.env.example` therefore exists as a documented template reserved for the
variables introduced by later phases.

## Template (`.env.example`)

```bash
# AI Gateway environment template.
# Copy to .env for local development: cp .env.example .env
# Values are placeholders only — never commit real secrets.

# Reserved namespace for gateway configuration.
# Example (future phase): AI_GATEWAY_LOG_LEVEL=<info|debug|warn|error>
# Example (future phase): AI_GATEWAY_PORT=<port>
```

## Rules

1. Every variable the application reads MUST be documented in `.env.example`.
2. `.env.example` MUST only ever contain placeholder values (FR-007, FR-012).
3. `.env` and other local files MUST remain untracked by version control
   (FR-008).
4. Later phases that introduce variables MUST update this contract before
   implementation.

## Maintenance

This contract is authoritative for configuration naming and documentation.
Changes MUST be reviewed against [privacy & secrets](../spec.md) requirements
(FR-012, SC-004).