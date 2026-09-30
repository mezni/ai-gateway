# Quickstart: OpenRouter Integration

## Prerequisites

1. **Rust toolchain**: Edition 2024, toolchain pinned 1.98.1
2. **OpenRouter API key**: Set the `OPENROUTER_API_KEY` environment variable
3. **Gateway built**: `cargo build --release`

## Setup

1. Ensure the OpenRouter provider is configured in `config/gateway.yaml`:
   ```yaml
   providers:
     openrouter:
       enabled: true
       base_url: "https://openrouter.ai/api/v1"
       api_key_env: "OPENROUTER_API_KEY"
   ```

2. Verify the API key is accessible:
   ```bash
   export OPENROUTER_API_KEY="sk-or-..."
   ```

## Running the Gateway

Start the gateway:

```bash
cargo run
```

The gateway should start and listen on the configured `AI_GATEWAY_HOST` and `AI_GATEWAY_PORT` (default: `127.0.0.1:8080`).

## Testing the Integration

### 1. Health Check

Verify the gateway is running:

```bash
curl -s http://127.0.0.1:8080/health
```

Expected: `{"code":"ok","message":"OK"}` or similar healthy response.

### 2. Readiness Check

```bash
curl -s http://127.0.0.1:8080/ready
```

Expected: `{"code":"ok","message":"Ready"}` if the gateway is ready to accept requests.

### 3. Chat Completion

Send a chat completion request to OpenRouter:

```bash
curl -s -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic/claude-3-opus-20240229",
    "messages": [
      {"role": "user", "content": "Hello, how are you?"}
    ]
  }'
```

Expected response: A valid chat completion response from OpenRouter with the model's reply.

### 4. Verify API Key Authentication

Send a request without the API key configured or with an invalid key:

```bash
curl -s -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic/claude-3-opus-20240229",
    "messages": [
      {"role": "user", "content": "Hello"}
    ]
  }'
```

Expected: Error response indicating authentication failure (HTTP 401 or gateway error contract `{"code":"authentication_error","message":"..."}`).

## Expected Outcomes

- Gateway successfully connects to OpenRouter and sends chat completion requests
- Valid API key allows successful provider communication
- Missing/invalid API key returns appropriate error
- Gateway handles provider errors gracefully without crashing
- Response mapping works correctly (OpenRouter format → internal ChatResponse model)

## Troubleshooting

- **Connection refused**: Verify the gateway is running and the port is correct
- **Authentication errors**: Check that `OPENROUTER_API_KEY` is set correctly
- **Rate limit errors**: The gateway has rate limiting; wait before retrying
- **Timeout errors**: Increase timeout configuration if OpenRouter responses are slow