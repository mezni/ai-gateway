# Changelog

All notable changes to the AI Gateway project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project documentation: PRD, Architecture, API, Configuration,
  Providers, Security, and Reliability docs under `docs/`
- README with architecture overview, phased roadmap, and documentation index
- Project structure with `docs/`, `specs/`, `docs/adr/`
- Spec Kit constitution (v1.0.0 ratified, v1.1.0 amended with
  `specs/` artifact requirement) and memory
- `CHANGELOG.md` and `HANDOFF.md`
- Phase 0 project foundation: zero-dependency Rust crate (Edition 2024,
  toolchain pinned via `rust-toolchain.toml`), `src/lib.rs` + `src/main.rs`
  entry point, `.gitignore`, `.env.example`, and README Development section
  with the quality-gate workflow (fmt/clippy/check/test/release)
- Spec Kit feature artifacts for `specs/001-project-foundation` (spec, plan,
  research, data model, contracts, tasks, quickstart)
- Spec Kit planning artifacts for `specs/002-layered-architecture` (spec,
  clarification for essential domain fields, plan, research, data model,
  module boundary contract, quickstart)
- Phase 1 layered architecture: lib-first module skeleton with the five
  layers (`domain`/`application`/`api`/`infrastructure`/`config`),
  provider-independent domain types (`ChatRequest`, `Message`, `MessageRole`,
  `ChatResponse`, `Usage`, `Model`, `Provider`), and an `AppState` composition
  root exercised by `main.rs`
- Spec Kit feature artifacts for `specs/003-http-gateway-core` ("HTTP Gateway
  Core", plan Phase 2): spec, plan, research, data model, contracts, tasks,
  and quickstart; 48/48 tasks complete with the requirements checklist at
  16/16
- Phase 2 HTTP gateway core: an Axum 0.8 server serving exactly three routes
  with the exact methods `GET /health`, `GET /ready`, and
  `POST /v1/chat/completions`
- Liveness probe `GET /health` returns 200 `{"status":"ok"}` in every
  lifecycle phase
- Readiness probe `GET /ready` returns 200 `{"status":"ready"}` only in the
  Ready phase, and 503 `{"status":"not_ready"}` in every other phase
- `POST /v1/chat/completions` backed by a stateless deterministic local mock:
  the request `{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}`
  returns exactly
  `{"id":"chat_mock","object":"chat.completion","model":"<echoed model>","choices":[{"index":0,"message":{"role":"assistant","content":"This is a mocked chat completion."},"finish_reason":"stop"}]}`
  with no `usage`, no `created`, no provider call, no credentials, no network
  egress, no persistence, and no streaming
- Chat option `stream` defaults to `false`; an explicit `stream: true` is
  rejected with 400 `unsupported_feature`
- Flat two-key error contract `{"code","message"}` with seven exact rows:
  400 `invalid_request` "The chat request is invalid."; 400
  `unsupported_feature` "Streaming is not supported."; 415
  `unsupported_media_type` "The request media type is not supported."; 405
  `method_not_allowed` "The request method is not allowed for this path."
  with an `Allow` header (`Allow: GET` or `Allow: POST`); 404 `not_found`
  "The requested path was not found."; 503 `not_ready` "The gateway is not
  accepting new chat requests."; 500 `internal_error` "The gateway could not
  complete the request."
- Error precedence: routing/method, then lifecycle admission, then media type,
  then JSON structure, then minimum validation, then streaming
- `HEAD` requests keep their status and headers while the body is suppressed
- Lifecycle state machine Initializing -> Ready (reached after the TCP bind
  succeeds) -> ShuttingDown -> Stopped
- Graceful shutdown on the first Ctrl-C or SIGTERM: `/health` stays 200,
  `/ready` returns 503, new chat requests get 503, already-admitted requests
  drain, then Axum shuts down gracefully, all under a single absolute
  10-second deadline measured from the original signal; on expiry the accept
  loop is aborted, the socket is released, and the process exits with a
  contextual error
- Configuration read from the process environment at startup:
  `AI_GATEWAY_HOST` (default 127.0.0.1) and `AI_GATEWAY_PORT` (default 3000);
  empty or malformed values and port 0 are startup failures, and no `.env`
  file is ever loaded automatically
- Gateway modules: `src/api/{server,health,chat,dto,error,middleware}.rs`,
  `src/application/{chat,lifecycle}.rs`, `src/domain/chat.rs`, and
  `src/config/server.rs`
- Dependencies added: Axum 0.8, Tokio 1 (features `io-util`, `macros`, `net`,
  `rt-multi-thread`, `signal`, `sync`, `time`), serde, serde_json, thiserror,
  anyhow, and tower as a dev-dependency for `ServiceExt::oneshot`; the
  zero-dependency policy ends with Phase 1
- Test suite expanded to 120 tests (87 library unit tests, 28 in
  `tests/http_api.rs`, 5 in `tests/server_lifecycle.rs`); startup, drain, and
  SIGTERM behavior were also verified by hand against a locally running
  binary

### Changed
- `AppState` composition root now carries the lifecycle state and the
  stateless `MockChatCompletionService`
- `main.rs` now binds the configured address, serves the gateway, and drives
  the lifecycle until shutdown completes
- Converted flow/architecture diagrams in markdown docs to Mermaid
- Aligned README target structure with architecture layering and
  YAML-based configuration in `docs/configuration.md`
- Replaced root `constitution.md` scaffold template with the ratified
  constitution text
- Normalized doc titles under `docs/` for consistency
- README: added "Current Module Structure" section describing the layered
  modules and the file-stem module layout

### Removed
- Reference to non-existent `docs/routing.md` and `docs/operations.md` in
  the README target structure

