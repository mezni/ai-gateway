# AI Gateway — Implementation Plan

## 1. Purpose

This document defines the incremental implementation roadmap for the Rust AI Gateway.

The project is intentionally built from a small working gateway into a production-oriented AI infrastructure platform.

The implementation order is:

```text
Foundation
    ↓
Gateway Core
    ↓
Provider Abstraction
    ↓
Model Routing
    ↓
Authentication
    ↓
Authorization
    ↓
Rate Limiting
    ↓
Reliability
    ↓
Streaming
    ↓
Observability
    ↓
Usage & Cost
    ↓
Persistence
    ↓
Policy Engine
    ↓
Caching
    ↓
Admin API
    ↓
Testing
    ↓
Security Hardening
    ↓
Performance
    ↓
Containerization
    ↓
CI/CD
    ↓
Production Deployment
```

The project should remain runnable after every major phase.

---

# 2. Development Philosophy

The gateway will be developed incrementally.

Each phase should introduce a small set of concepts and produce a working result.

Every phase follows:

```text
Learn
  ↓
Design
  ↓
Specify
  ↓
Implement
  ↓
Test
  ↓
Run
  ↓
Observe
  ↓
Document
```

Do not implement advanced infrastructure before the underlying concept is understood.

For example:

```text
Do not start with:
Kubernetes + Redis + PostgreSQL + multiple providers

Start with:
Axum + Rust + one provider + one endpoint
```

---

# 3. Technology Stack

## Core

```text
Rust
Cargo
Tokio
Axum
Serde
serde_json
thiserror
anyhow
```

## HTTP

```text
Axum
Tower
Reqwest
```

## Async

```text
Tokio
```

## Persistence

```text
PostgreSQL
SQLx
```

## Distributed State

```text
Redis
```

## Observability

```text
tracing
tracing-subscriber
OpenTelemetry
Prometheus-compatible metrics
```

## Testing

```text
cargo test
integration tests
mock providers
property-based testing where useful
load testing later
```

## Infrastructure

```text
Docker
Docker Compose
GitHub Actions
```

---

# 4. Repository Structure

The final project is expected to evolve toward:

```text
ai-gateway/
│
├── Cargo.toml
├── Cargo.lock
├── README.md
├── constitution.md
├── .gitignore
├── .env.example
├── Dockerfile
├── docker-compose.yml
├── Makefile
│
├── config/
│   ├── gateway.yaml
│   ├── models.yaml
│   └── development.yaml
│
├── docs/
│   ├── prd.md
│   ├── architecture.md
│   ├── plan.md
│   ├── api.md
│   ├── configuration.md
│   ├── providers.md
│   ├── security.md
│   ├── reliability.md
│   ├── observability.md
│   ├── testing.md
│   ├── deployment.md
│   └── adr/
│
├── specs/
│   ├── 001-gateway-core/
│   ├── 002-provider-abstraction/
│   ├── 003-model-routing/
│   ├── 004-authentication/
│   ├── 005-authorization/
│   ├── 006-rate-limiting/
│   ├── 007-reliability/
│   ├── 008-streaming/
│   ├── 009-observability/
│   ├── 010-usage-tracking/
│   ├── 011-cost-management/
│   ├── 012-persistence/
│   ├── 013-policy-engine/
│   ├── 014-caching/
│   ├── 015-admin-api/
│   ├── 016-security-hardening/
│   ├── 017-performance/
│   └── 018-production-deployment/
│
├── migrations/
│
├── tests/
│   ├── integration/
│   └── fixtures/
│
└── src/
    ├── main.rs
    │
    ├── api/
    │
    ├── application/
    │
    ├── domain/
    │
    ├── infrastructure/
    │
    └── config/
```

The structure should evolve naturally rather than being created completely on day one.

---

# 5. Phase 0 — Project Foundation

## Objective

Create a clean Rust project and establish the development workflow.

## Status

**COMPLETE** (2026-09-24) — see `specs/001-project-foundation/`.

- Zero-dependency Rust crate created (Edition 2024, toolchain pinned 1.98.1)
- `src/lib.rs` + `src/main.rs` entry point; application runs and exits 0
- `.gitignore`, `.env.example`, `README.md` Development section added
- All quality gates pass (fmt --check, clippy -D warnings, test, build)
- Quickstart validation scenarios verified end to end

## Learn

* Rust project structure
* Cargo
* modules
* ownership
* borrowing
* `Result`
* `Option`
* async basics
* Tokio basics
* environment variables

## Build

Create:

```text
Cargo.toml
src/main.rs
.gitignore
.env.example
README.md
```

Add the initial dependencies.

Run:

```bash
cargo check
cargo test
cargo fmt
cargo clippy
```

## Deliverable

A minimal Rust application that compiles and passes tests.

---

# 6. Phase 1 — Architecture Specification

## Objective

Translate the architecture into implementable boundaries.

## Learn

* layered architecture
* dependency inversion
* domain/application/infrastructure separation
* traits
* dependency injection
* application state

## Define

```text
domain/
application/
infrastructure/
api/
config/
```

Define initial domain types:

```text
ChatRequest
Message
MessageRole
ChatResponse
Usage
Model
Provider
```

## Deliverable

Architecture skeleton with no real provider dependency.

**COMPLETE** (2026-09-24) — see `specs/002-layered-architecture/`.

---

# 7. Phase 2 — HTTP Gateway Core

## Objective

Build the first working HTTP gateway.

## Status

**COMPLETE** (2026-09-25) — see `specs/003-http-gateway-core/`.

- Three endpoints delivered with the exact methods above and no others
  (`GET /health`, `GET /ready`, `POST /v1/chat/completions`)
- The mocked chat endpoint shipped as planned: a stateless deterministic
  local completion, with no provider call, credentials, network egress,
  persistence, or streaming
- Flat two-key error contract `{"code","message"}` applied uniformly to
  routing, media type, validation, lifecycle, and internal failures
- Readiness is driven by the lifecycle (Initializing → Ready → ShuttingDown
  → Stopped), and shutdown is graceful under a single absolute 10-second
  deadline
- Configuration read from the process environment (`AI_GATEWAY_HOST`,
  `AI_GATEWAY_PORT`); no `.env` file is loaded automatically
- All quality gates pass (fmt --check, clippy -D warnings, check, test,
  build); startup, drain, and SIGTERM verified against a running binary

## Learn

* Axum
* routes
* handlers
* extractors
* JSON
* middleware
* HTTP status codes

## Endpoints

```text
GET /health
GET /ready
POST /v1/chat/completions
```

Initially the chat endpoint can return a mocked response.

Example:

```text
Client
  |
  v
Axum
  |
  v
Chat Handler
  |
  v
Mock Response
```

## Deliverable

A running gateway accessible through HTTP.

---

# 8. Phase 3 — Domain Models and Validation

## Objective

Create a stable internal request/response model.

## Learn

* Serde
* serialization/deserialization
* validation
* domain modeling
* error types

Define:

```rust
ChatRequest
Message
MessageRole
ChatResponse
Usage
```

Validate:

```text
model
messages
temperature
max_tokens
stream
request size
```

## Deliverable

Validated chat request entering the application layer.

---

# 9. Phase 4 — Provider Abstraction

## Objective

Separate gateway logic from provider-specific APIs.

## Learn

* Rust traits
* async traits
* trait objects
* `Arc`
* dependency injection
* adapters

Define:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError>;
}
```

Implement:

```text
MockProvider
OpenRouterProvider
```

Architecture:

```text
Chat Service
     |
     v
LlmProvider
     |
     +---- MockProvider
     |
     +---- OpenRouterProvider
```

## Deliverable

The gateway can communicate with OpenRouter through the provider abstraction.

---

# 10. Phase 5 — OpenRouter Integration

## Objective

Connect the gateway to a real LLM provider.

## Learn

* Reqwest
* HTTP clients
* authentication headers
* provider request mapping
* provider response mapping
* external API failures

Configuration:

```yaml
providers:
  openrouter:
    enabled: true
    base_url: "https://openrouter.ai/api/v1"
    api_key_env: "OPENROUTER_API_KEY"
```

The API key must come from the environment.

## Deliverable

```text
Client
  ↓
AI Gateway
  ↓
LlmProvider
  ↓
OpenRouter
  ↓
LLM
```

---

# 11. Phase 6 — Model Registry and Routing

## Objective

Allow clients to use gateway model names rather than provider-specific implementation details.

Example:

```text
client model:
default-chat
```

Mapped internally to:

```text
provider: openrouter
provider_model: some-model
```

## Learn

* registries
* aliases
* routing decisions
* configuration-driven behavior

Define:

```rust
pub struct RoutingDecision {
    pub provider: String,
    pub model: String,
}
```

## Deliverable

A model can be routed through a configured provider.

---

# 12. Phase 7 — Authentication

## Objective

Protect the gateway using API keys.

## Learn

* authentication
* middleware
* bearer tokens
* identity context
* secret handling

Request:

```http
Authorization: Bearer <API_KEY>
```

Create:

```rust
pub struct Identity {
    pub api_key_id: String,
    pub tenant_id: String,
    pub permissions: Vec<Permission>,
}
```

## Deliverable

Protected endpoints reject unauthenticated requests.

---

# 13. Phase 8 — Authorization

## Objective

Determine what authenticated identities are allowed to do.

Permissions:

```text
chat:execute
models:read
usage:read
admin:read
admin:write
```

Flow:

```text
Authentication
      ↓
Identity
      ↓
Authorization
      ↓
Application operation
```

## Deliverable

Authentication and authorization are separate concerns.

---

# 14. Phase 9 — Rate Limiting

## Objective

Protect the gateway and providers from excessive traffic.

Initial implementation:

```text
in-memory rate limiter
```

Later:

```text
Redis distributed rate limiter
```

Potential limits:

```text
requests / second
requests / minute
tokens / minute
```

## Deliverable

Requests exceeding configured limits receive a normalized rate-limit response.

---

# 15. Phase 10 — Reliability

## Objective

Make provider execution resilient.

Implement in this order:

```text
Timeout
  ↓
Failure classification
  ↓
Retry
  ↓
Exponential backoff
  ↓
Jitter
  ↓
Request deadline
  ↓
Cancellation
  ↓
Fallback
  ↓
Circuit breaker
```

## Deliverable

A provider outage does not automatically become an uncontrolled gateway outage.

---

# 16. Phase 11 — Streaming

## Objective

Support streaming responses.

API:

```text
POST /v1/chat/completions
stream = true
```

Architecture:

```text
Provider Stream
      |
      v
Gateway Stream
      |
      v
SSE
      |
      v
Client
```

Handle:

```text
disconnects
timeouts
cancellation
backpressure
partial responses
stream errors
usage accounting
```

## Deliverable

The gateway can stream provider responses to clients.

---

# 17. Phase 12 — Observability

## Objective

Make the gateway observable.

## Logging

Use:

```text
tracing
tracing-subscriber
```

Log:

```text
request_id
provider
model
latency
status
error_type
attempt
```

Never log:

```text
API keys
Authorization headers
full prompts
full responses
```

## Metrics

Add:

```text
request count
error count
latency
provider latency
timeouts
retries
fallbacks
active requests
```

## Tracing

Introduce:

```text
OpenTelemetry
```

Trace:

```text
HTTP request
  |
  +-- authentication
  |
  +-- routing
  |
  +-- provider call
  |
  +-- retry
  |
  +-- response
```

## Deliverable

A failed request can be investigated through logs, metrics, and traces.

---

# 18. Phase 13 — Usage Tracking

## Objective

Track model usage.

Capture:

```text
prompt tokens
completion tokens
total tokens
request duration
provider
model
tenant
request ID
```

Example:

```rust
pub struct UsageRecord {
    pub request_id: String,
    pub tenant_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}
```

## Deliverable

Every completed provider request produces normalized usage information when the provider supplies usage data.

---

# 19. Phase 14 — Cost Management

## Objective

Calculate estimated request costs.

Pricing configuration:

```yaml
pricing:
  model-name:
    input_per_million_tokens: 0.0
    output_per_million_tokens: 0.0
```

Calculate:

```text
input cost
output cost
total cost
```

Cost must remain separate from usage.

```text
Usage
  |
  v
Pricing
  |
  v
Cost
```

## Deliverable

The gateway can estimate request cost using configurable pricing.

---

# 20. Phase 15 — PostgreSQL Persistence

## Objective

Persist gateway state and operational data.

## Learn

* PostgreSQL
* SQLx
* migrations
* transactions
* connection pools
* repository patterns

Initial tables:

```text
tenants
api_keys
providers
models
requests
usage_records
audit_events
```

Use SQLx migrations.

## Deliverable

Persistent gateway metadata and usage records.

---

# 21. Phase 16 — Redis

## Objective

Introduce distributed ephemeral state.

Redis will eventually support:

```text
distributed rate limits
quotas
short-lived cache
distributed coordination
```

Redis should not become the system of record.

Architecture:

```text
PostgreSQL
    |
persistent state

Redis
    |
ephemeral/distributed state
```

## Deliverable

Distributed rate limiting and other appropriate ephemeral state.

---

# 22. Phase 17 — Policy Engine

## Objective

Introduce configurable request policies.

Policies may eventually control:

```text
allowed models
allowed providers
maximum tokens
maximum cost
tenant restrictions
streaming permissions
rate limits
data handling requirements
```

Flow:

```text
Identity
   ↓
Policy Engine
   ↓
Policy Decision
   ↓
Router
```

## Deliverable

Requests can be allowed or rejected according to explicit policies.

---

# 23. Phase 18 — Caching

## Objective

Introduce optional response caching.

Potential architecture:

```text
Request
  |
  v
Cache lookup
  |
  +---- hit ----> Response
  |
  +---- miss
          |
          v
       Provider
          |
          v
        Cache
          |
          v
       Response
```

Caching must consider:

```text
tenant isolation
model
request parameters
prompt content
policy
streaming
TTL
cache invalidation
```

Caching should be disabled by default until correctness and privacy requirements are defined.

---

# 24. Phase 19 — Admin API

## Objective

Provide controlled operational management.

Potential endpoints:

```text
/admin/providers
/admin/models
/admin/tenants
/admin/api-keys
/admin/policies
/admin/usage
```

Admin operations require:

```text
authentication
authorization
audit logging
```

## Deliverable

Controlled administrative management without exposing internal infrastructure directly.

---

# 25. Phase 20 — Testing Strategy

## Objective

Build confidence in the system.

Testing layers:

```text
Unit Tests
    ↓
Component Tests
    ↓
Integration Tests
    ↓
Provider Contract Tests
    ↓
End-to-End Tests
    ↓
Failure Tests
    ↓
Load Tests
```

## Unit Tests

Test:

```text
routing
validation
error classification
retry policy
cost calculation
policy evaluation
```

## Integration Tests

Test:

```text
HTTP → application → provider
```

## Mock Provider

Support deterministic behavior:

```text
success
timeout
network failure
rate limit
server error
invalid response
```

## Deliverable

Reliability behavior is tested without requiring real provider failures.

---

# 26. Phase 21 — Security Hardening

## Objective

Strengthen security after core behavior is stable.

Implement:

```text
request size limits
secure headers
CORS policy
strict input validation
secret protection
admin isolation
tenant isolation
audit logging
dependency auditing
container security
```

Security testing:

```text
authentication tests
authorization tests
tenant isolation tests
secret leakage tests
request validation tests
SSRF protection tests
rate-limit tests
```

## Deliverable

Security controls are verified through automated tests.

---

# 27. Phase 22 — Performance

## Objective

Measure and improve gateway performance.

Measure:

```text
request latency
provider latency
gateway overhead
throughput
concurrent requests
memory usage
CPU usage
connection pool behavior
```

Test:

```text
100 concurrent requests
500 concurrent requests
1000 concurrent requests
```

Actual limits should be determined from measurements.

## Deliverable

A measured performance profile rather than assumed performance characteristics.

---

# 28. Phase 23 — Docker

## Objective

Create reproducible local infrastructure.

Containers:

```text
AI Gateway
PostgreSQL
Redis
```

Example:

```text
docker compose up
```

The gateway should be configurable through environment variables.

## Deliverable

A new developer can start the local platform with one command.

---

# 29. Phase 24 — CI/CD

## Objective

Automate quality checks.

Pipeline:

```text
Push
  ↓
Format
  ↓
Clippy
  ↓
Unit Tests
  ↓
Integration Tests
  ↓
Security Checks
  ↓
Build
  ↓
Docker Image
```

Recommended checks:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Later add:

```text
dependency auditing
container scanning
integration environment
```

## Deliverable

Every pull request receives automated validation.

---

# 30. Phase 25 — Production Deployment

## Objective

Prepare the gateway for production operation.

Production architecture:

```text
                    Load Balancer
                         |
          +--------------+--------------+
          |              |              |
          v              v              v
      Gateway        Gateway        Gateway
          |              |              |
          +--------------+--------------+
                         |
              +----------+----------+
              |                     |
              v                     v
          PostgreSQL              Redis
              |
              |
        External Providers
```

Gateway instances should be stateless where practical.

Production concerns:

```text
TLS
secrets
health checks
readiness
graceful shutdown
autoscaling
logging
metrics
tracing
database backups
failure recovery
monitoring
alerting
```

---

# 31. SpecKit Workflow

Every major feature should use the SpecKit workflow.

```text
Constitution
     ↓
Feature Specification
     ↓
Implementation Plan
     ↓
Tasks
     ↓
Implementation
     ↓
Tests
     ↓
Documentation
```

For example:

```text
specs/
└── 001-gateway-core/
    ├── spec.md
    ├── plan.md
    └── tasks.md
```

The feature should not be implemented before its specification is clear.

---

# 32. Feature Order

The SpecKit features should be implemented in this order:

```text
001 gateway-core
        ↓
002 provider-abstraction
        ↓
003 model-routing
        ↓
004 authentication
        ↓
005 authorization
        ↓
006 rate-limiting
        ↓
007 reliability
        ↓
008 streaming
        ↓
009 observability
        ↓
010 usage-tracking
        ↓
011 cost-management
        ↓
012 persistence
        ↓
013 policy-engine
        ↓
014 caching
        ↓
015 admin-api
        ↓
016 security-hardening
        ↓
017 performance
        ↓
018 production-deployment
```

---

# 33. MVP Definition

The MVP should be deliberately small.

## MVP includes

```text
Rust
Tokio
Axum
Serde
Reqwest

/health
/ready
/v1/chat/completions

Chat domain models
Request validation

LlmProvider trait
MockProvider
OpenRouterProvider

Model registry
Basic routing

Basic API-key authentication
Basic authorization

Timeouts
Basic retries
Normalized errors

Structured logging
Request IDs

Unit tests
Integration tests
```

## MVP excludes

```text
PostgreSQL
Redis
Distributed rate limiting
Complex policy engine
Caching
Admin API
Circuit breakers
Advanced routing
Kubernetes
Complex cost analytics
Advanced distributed tracing
```

The goal is to get a complete working vertical slice first.

---

# 34. First Vertical Slice

The first complete system should look like:

```text
                   Client
                     |
                     | POST /v1/chat/completions
                     v
                  Axum API
                     |
                     v
              Authentication
                     |
                     v
                 Chat Service
                     |
                     v
                Model Router
                     |
                     v
               LlmProvider
                     |
                     v
             OpenRouter Provider
                     |
                     v
                 OpenRouter
                     |
                     v
                  Response
                     |
                     v
                   Client
```

This is the most important milestone.

Before adding infrastructure, the basic request path must work.

---

# 35. Definition of Done

A phase is complete when:

```text
[ ] Design is documented
[ ] SpecKit specification exists
[ ] Implementation is complete
[ ] Unit tests pass
[ ] Integration tests pass where applicable
[ ] cargo fmt passes
[ ] cargo clippy passes
[ ] Application runs locally
[ ] Failure behavior is understood
[ ] Documentation is updated
```

A feature should not be considered complete merely because the code compiles.

---

# 36. Learning Checkpoints

The project is also a Rust and AI-infrastructure learning project.

After each phase, understand the following.

## Foundation

```text
Why Tokio?
Why Result?
How does Cargo work?
How does async Rust work?
```

## Gateway

```text
How does Axum route requests?
How does middleware work?
How does application state work?
```

## Providers

```text
Why use traits?
Why dependency inversion?
Why normalize provider APIs?
```

## Routing

```text
What is a routing decision?
Why separate model aliases from provider models?
```

## Security

```text
What is authentication?
What is authorization?
Why separate identity from permissions?
```

## Reliability

```text
Why retry?
When should we not retry?
Why use backoff?
Why use circuit breakers?
```

## Observability

```text
What is a log?
What is a metric?
What is a trace?
How are they correlated?
```

## Persistence

```text
What belongs in PostgreSQL?
What belongs in Redis?
```

## Production

```text
How does horizontal scaling work?
What makes an application stateless?
How do health checks work?
How does graceful shutdown work?
```

---

# 37. Final Architecture

After all major phases, the system should evolve toward:

```text
                           Clients
                              |
                              v
                       Load Balancer
                              |
             +----------------+----------------+
             |                |                |
             v                v                v
        AI Gateway       AI Gateway       AI Gateway
             |                |                |
             +----------------+----------------+
                              |
                              v
                         API Layer
                              |
                              v
                    Authentication
                              |
                              v
                     Authorization
                              |
                              v
                     Rate Limiting
                              |
                              v
                         Quotas
                              |
                              v
                    Policy Engine
                              |
                              v
                      Model Router
                              |
                    +---------+---------+
                    |         |         |
                    v         v         v
                Provider A Provider B Provider C
                    |         |         |
                    +---------+---------+
                              |
                              v
                         Reliability
                              |
             +----------------+----------------+
             |                |                |
          Timeout           Retry           Fallback
             |                |                |
             +----------------+----------------+
                              |
                              v
                         LLM Providers

                              |
              +---------------+---------------+
              |               |               |
              v               v               v
          PostgreSQL        Redis        Observability
              |               |               |
              v               v               v
          Persistent      Distributed      Logs
             Data            State         Metrics
                                            Traces
```

---

# 38. Long-Term Outcome

The completed project should demonstrate the ability to build an AI infrastructure service in Rust from first principles.

The progression is:

```text
Rust fundamentals
       ↓
Async programming
       ↓
HTTP services
       ↓
AI provider integration
       ↓
Abstraction
       ↓
Routing
       ↓
Security
       ↓
Reliability
       ↓
Observability
       ↓
Persistence
       ↓
Distributed infrastructure
       ↓
Testing
       ↓
Performance
       ↓
Deployment
       ↓
Production-oriented AI Gateway
```

The project is not intended to become a general-purpose agent framework or model-serving system.

Its primary responsibility is:

> Provide a reliable, secure, observable, provider-independent gateway for applications consuming LLM services.