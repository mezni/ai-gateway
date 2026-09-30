# Feature Specification: OpenRouter Integration

**Feature Branch**: `[006-openrouter-integration]`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: "read from docs/plan.md phase 5"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect to OpenRouter LLM (Priority: P1)

[Describe this user journey in plain language]

**Why this priority**: Users need to connect the AI Gateway to an LLM provider via OpenRouter to send chat completions requests and receive responses.

**Independent Test**: The gateway can successfully send a chat completion request to OpenRouter and receive a valid response using a valid API key.

**Acceptance Scenarios**:

1. **Given** a configured OpenRouter provider with a valid API key, **When** the gateway sends a chat completion request, **Then** the request is routed to OpenRouter and a response is received.

2. **Given** no API key is configured, **When** the gateway sends a chat completion request, **Then** the gateway returns an error indicating the API key is missing.

3. **Given** an invalid API key, **When** the gateway sends a chat completion request, **Then** the gateway returns an authentication error from OpenRouter.

### Clarifications

**Session 2026-09-30**

- Q: Performance target → A: Users see response in under 5 seconds for 95% of requests
- Q: User roles → A: Support two roles: "user" (chat requestors) and "admin" (operational management)
- Q: Error handling → A: Map all OpenRouter errors to the gateway's flat two-key contract `{"code","message"}` with normalized codes
- Q: Observability telemetry → A: Emit request_id, provider, model, latency, status, error_type, attempt, and request count metrics
- Q: Security & privacy → A: Prohibit logging API keys/authorization headers; mask prompts in telemetry; require PII handling per domain policy

<tool_call>

### Edge Cases

- What happens when the OpenRouter API is unreachable?
- How does the gateway handle malformed provider responses?
- What behavior occurs when the request exceeds token limits?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The gateway MUST support connecting to OpenRouter as an LLM provider
- **FR-002**: The gateway MUST send chat completion requests to OpenRouter with proper authentication headers
- **FR-003**: The gateway MUST map internal request models to OpenRouter API format
- **FR-004**: The gateway MUST map OpenRouter API responses to internal response models
- **FR-005**: The gateway MUST handle external API failures gracefully without crashing, mapping all errors to the gateway's flat two-key contract `{"code","message"}`

*Example of marking unclear requirements:*

- **FR-006**: System MUST authenticate with OpenRouter using API key from environment variable

### Key Entities

- **ChatRequest**: Represents the user's chat request with messages, model selection, and parameters
- **ChatResponse**: Represents the LLM provider's response with content, usage information, and model name
- **ProviderConfiguration**: Holds provider-specific settings including API key, base URL, and enabled status

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can complete a chat completion request through the gateway to OpenRouter
- **SC-002**: The gateway returns valid LLM responses for supported model requests
- **SC-003**: The gateway handles authentication errors when API key is missing or invalid
- **SC-004**: System supports configurable OpenRouter provider settings (base URL, API key environment variable)

## Assumptions

- Users have valid OpenRouter API keys configured via the `OPENROUTER_API_KEY` environment variable
- The OpenRouter API follows the standard OpenAI-compatible chat completion format
- Network connectivity to OpenRouter's endpoint (`https://openrouter.ai/api/v1`) is available
- Provider configuration is externalized and validated at startup
- The gateway's internal chat request/response models remain provider-agnostic
- Latency target: 95% of OpenRouter responses returned within 5 seconds
- Two user roles supported: "user" (chat requestors) and "admin" (operational management)
- Telemetry signals: request_id, provider, model, latency, status, error_type, attempt, and request count metrics
- Security & privacy: prohibit logging API keys/authorization headers; mask prompts in telemetry; require PII handling per domain policy

---