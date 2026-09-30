# API Contracts: OpenRouter Integration

## Chat Completion Request

**Endpoint**: `POST /v1/chat/completions`

**Description**: Send a chat completion request to OpenRouter.

### Request Format

```json
{
  "model": "string",
  "messages": [
    {
      "role": "system | user | assistant",
      "content": "string"
    }
  ],
  "temperature": number,          // optional, default: provider default
  "max_tokens": integer,          // optional, default: provider default
  "stream": boolean               // optional, default: false
}
```

**Required fields**:
- `model`: The OpenRouter model identifier (e.g., "anthropic/claude-3-opus-20240229")
- `messages`: Array of message objects with `role` and `content`

**Optional fields**:
- `temperature`: Sampling temperature in range [0.0, 2.0]
- `max_tokens`: Maximum tokens in completion
- `stream`: Whether to stream the response

### Response Format

```json
{
  "id": "string",
  "object": "chat.completion",
  "created": integer,
  "model": "string",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "string"
      },
      "finish_reason": "string"
    }
  ],
  "usage": {
    "prompt_tokens": integer,
    "completion_tokens": integer,
    "total_tokens": integer
  }
}
```

### Error Response Format

The gateway normalizes all provider errors to its flat two-key contract:

```json
{
  "code": "string",
  "message": "string"
}
```

**Possible error codes**:
- `authentication_error`: Invalid or missing API key
- `rate_limit_error`: Rate limit exceeded
- `provider_error`: General provider failure
- `validation_error`: Invalid request format
- `not_found_error`: Model not found

## Provider Configuration Contract

**Endpoint**: Configuration via `config/gateway.yaml`

**Format**:
```yaml
providers:
  openrouter:
    enabled: boolean
    base_url: string
    api_key_env: string
```

**Fields**:
- `enabled`: Whether the provider is active
- `base_url`: API base endpoint
- `api_key_env`: Environment variable name containing the API key