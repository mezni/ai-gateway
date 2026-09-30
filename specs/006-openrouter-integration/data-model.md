# Data Model: OpenRouter Integration

## Entities from Feature Spec

### ChatRequest

Represents the user's chat request to be sent to the LLM provider.

**Fields**:
- `messages`: Vec<Message> - Conversation history with role and content
- `model`: String - Selected model name (optional, may use default)
- `temperature`: f32 - Sampling temperature (0.0-2.0, optional, uses provider default if not specified)
- `max_tokens`: u32 - Maximum tokens in response (optional, uses provider default if not specified)
- `stream`: bool - Whether to request streaming response (default: false)

### Message

Individual message in the conversation.

**Fields**:
- `role`: MessageRole - Role of the message sender (user, assistant, system)
- `content`: String - Message content

### MessageRole

Enumeration of message roles.

**Variants**:
- `System`: System/instructions message
- `User`: User message
- `Assistant`: Assistant/AI response

### ChatResponse

Represents the LLM provider's response.

**Fields**:
- `content`: String - The generated response content
- `role`: MessageRole - Role of the response (typically "assistant")
- `model`: String - Name of the model that generated the response
- `usage`: UsageRecord - Token usage information

### UsageRecord

Token usage information from the provider.

**Fields**:
- `prompt_tokens`: u64 - Number of prompt tokens
- `completion_tokens`: u64 - Number of completion tokens
- `total_tokens`: u64 - Total tokens (prompt + completion)

### ProviderConfiguration

Holds provider-specific settings.

**Fields**:
- `enabled`: bool - Whether the provider is enabled (default: true)
- `base_url`: String - API base URL (default: "https://openrouter.ai/api/v1")
- `api_key_env`: String - Environment variable containing the API key (default: "OPENROUTER_API_KEY")

## Relationships

- **ChatRequest** → **Message** (contains, 1:N)
- **ChatRequest** → **ChatResponse** (request/response pair)
- **ProviderConfiguration** → **ChatRequest** (provides settings for request construction)
- **ChatResponse** → **UsageRecord** (contains usage information)

## Validation Rules

- **Message.content** must not be empty after trimming
- **Message.role** must be a valid MessageRole variant
- **temperature** if specified must be in range [0.0, 2.0]
- **max_tokens** if specified must be in range [1, 4096]
- **model** if specified must not be empty
- Request must contain at least one message

## State Transitions

- **Idle** → **Sending** → **Received** → **Idle** (per request cycle)
- **Idle** → **Error** (if request fails) → **Idle** (recovered state)
- Configuration update: **Disabled** → **Enabled** (when provider is toggled on)