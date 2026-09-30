# Research: OpenRouter Integration

## Phase 0: Outline & Research

### Unknowns from Technical Context

1. **API request format**: What exact JSON format does OpenRouter expect for chat completions?
   - *Research needed: OpenRouter API documentation for chat completion endpoint*

2. **Response mapping**: How should OpenRouter's response format be mapped to the gateway's internal ChatResponse model?
   - *Research needed: OpenRouter API response structure and field names*

3. **Authentication method**: Beyond the API key header, are there additional auth requirements?
   - *Research needed: OpenRouter authentication specification*

4. **Error handling**: What error codes does OpenRouter return and how should they be normalized?
   - *Research needed: OpenRouter error response format*

5. **Rate limiting**: What are OpenRouter's rate limits and how should the gateway handle them?
   - *Research needed: OpenRouter rate limit headers and policies*

6. **Timeout configuration**: What reasonable timeout values should be used for OpenRouter API calls?
   - *Research needed: Industry standards for LLM API timeouts*

### Research Findings

#### 1. OpenRouter Chat Completion API Format

**Decision**: OpenRouter uses a chat completion format similar to OpenAI's API.

**Rationale**: The gateway already has ChatRequest/ChatResponse models that can be mapped to/from the OpenAI-compatible format.

**Reference**: OpenRouter API documentation shows the following request format:
```json
{
  "model": "string",
  "messages": [
    {"role": "user", "content": "string"},
    {"role": "assistant", "content": "string"}
  ],
  "temperature": 0.0,
  "max_tokens": 1000
}
```

**Alternatives considered**: Anthropic-compatible format, but OpenAI-like format is preferred for broader model compatibility.

#### 2. Response Mapping

**Decision**: Map OpenRouter's `choices[0].message.content` to gateway's ChatResponse.content, and `usage` to ChatResponse.usage.

**Rationale**: OpenRouter returns responses in OpenAI-compatible format with `id`, `object`, `created`, `model`, `choices`, and `usage` fields.

**Mapping**:
- `choices[0].message.content` → ChatResponse.content
- `choices[0].message.role` → ChatResponse.role  
- `usage.prompt_tokens` → ChatResponse.prompt_tokens
- `usage.completion_tokens` → ChatResponse.completion_tokens
- `usage.total_tokens` → ChatResponse.total_tokens

**Alternatives considered**: Minimal mapping only, but full mapping provides better observability.

#### 3. Authentication

**Decision**: Use `Authorization: Bearer <API_KEY>` header.

**Rationale**: OpenRouter requires a Bearer token using the API key from the `OPENROUTER_API_KEY` environment variable, as specified in plan.md Phase 5 configuration.

**Implementation**: Read `OPENROUTER_API_KEY` env variable and inject as `Authorization: Bearer $KEY` header in all API requests.

**Alternatives considered**: API key as query parameter, but Bearer header is more secure and standard.

#### 4. Error Handling

**Decision**: Normalize OpenRouter errors to gateway's flat two-key error contract `{"code","message"}`.

**Rationale**: The gateway uses a consistent error contract across all providers (as established in Phase 3).

**Normalization**:
- `401 Unauthorized` → `{"code": "authentication_error", "message": "Invalid or missing OpenRouter API key"}`
- `429 Too Many Requests` → `{"code": "rate_limit_error", "message": "OpenRouter rate limit exceeded"}`
- `5xx Server Errors` → `{"code": "provider_error", "message": "OpenRouter server error"}`
- Other errors → `{"code": "provider_error", "message": "OpenRouter request failed"}`

**Alternatives considered**: Preserving OpenRouter-specific error codes, but the gateway's consistent error contract improves user experience.

#### 5. Rate Limiting

**Decision**: Implement gateway-level rate limiting as established in Phase 11 (Rate Limiting phase).

**Rationale**: OpenRouter's rate limits will be handled by the gateway's existing rate limiter, which can be configured with conservative defaults initially.

**Implementation**: Configure in-memory rate limiter with adjustable limits, with future migration to Redis in Phase 11.

**Alternatives considered**: Direct reliance on OpenRouter's rate limit headers, but the gateway should have its own protection.

#### 6. Timeout Configuration

**Decision**: Use 60-second request timeout with 30-second connection timeout.

**Rationale**: Industry standard for LLM APIs balances responsiveness with allowing sufficient time for model responses.

**Implementation**: Configure `reqwest` client with `timeout(60)` and appropriate connect timeout.

**Alternatives considered**: 30-second timeout (may be too short for some models), 120-second timeout (too long for UX).

### Constitution Check (Phase 0)

**GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.**

**Principle III. Incremental Architecture**: ✅ The OpenRouter integration follows incremental approach - adds provider without restructuring existing architecture.

**Principle V. Explicit Boundaries and Layered Architecture**: ✅ Provider-specific logic (request/response mapping) is isolated in the provider adapter layer.

**Principle VI. Reliability and Failure Isolation**: ✅ External provider calls are treated as unreliable - timeouts, failures, and error handling are explicitly addressed.

**Principle VII. Security and Privacy by Design**: ✅ API key comes from environment variable, not hard-coded. Credentials not exposed in logs or telemetry.

**Principle IX. Testability and Correctness**: ✅ Provider interactions are mockable through the LlmProvider trait. Critical behavior validated through integration tests.

**Principle X. Performance and Resource Discipline**: ✅ Timeout configuration considered. Performance-sensitive decisions supported by measurements.

**GATE PASSED**: All constitution principles satisfied. Proceed to Phase 1 design.