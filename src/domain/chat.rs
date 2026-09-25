//! Chat domain concepts: requests, messages, responses, and usage accounting.

use std::fmt;

/// The role a [`Message`] plays in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// System/instructions messages.
    System,
    /// End-user messages.
    User,
    /// Assistant/model replies.
    Assistant,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        };
        f.write_str(s)
    }
}

/// A single exchange within a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Role of the message author.
    pub role: MessageRole,
    /// Text payload of the message.
    pub content: String,
}

impl Message {
    /// Creates a new message with the given role and content.
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// A client's request for a chat completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRequest {
    /// Model reference the request targets.
    pub model: String,
    /// Messages to send, in conversation order.
    pub messages: Vec<Message>,
}

impl ChatRequest {
    /// Creates a chat request for the given model and messages.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
        }
    }
}

/// Token accounting for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens.
    pub prompt_tokens: u64,
    /// Output tokens.
    pub completion_tokens: u64,
    /// Total tokens (prompt + completion).
    pub total_tokens: u64,
}

impl Usage {
    /// Creates usage accounting, enforcing total = prompt + completion.
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// The gateway's normalized response to a chat request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    /// Response text.
    pub content: String,
    /// Token accounting for the request.
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// Creates a chat response with the given content and usage.
    pub fn new(content: impl Into<String>, usage: Usage) -> Self {
        Self {
            content: content.into(),
            usage: Some(usage),
        }
    }

    pub fn without_usage(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            usage: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_construction() {
        let request = ChatRequest::new(
            "test-model",
            vec![
                Message::new(MessageRole::System, "You are helpful."),
                Message::new(MessageRole::User, "Hello!"),
            ],
        );
        assert_eq!(request.model, "test-model");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, MessageRole::System);
        assert_eq!(request.messages[0].content, "You are helpful.");
        assert_eq!(request.messages[1].role, MessageRole::User);
    }

    #[test]
    fn message_role_variants_cover_system_user_assistant() {
        assert_eq!(format!("{}", MessageRole::System), "system");
        assert_eq!(format!("{}", MessageRole::User), "user");
        assert_eq!(format!("{}", MessageRole::Assistant), "assistant");
    }

    #[test]
    fn usage_accounting_enforces_total_equals_prompt_plus_completion() {
        let usage = Usage::new(10, 5);
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
    }

    #[test]
    fn chat_response_new_preserves_measured_usage() {
        let usage = Usage::new(2, 3);
        let response = ChatResponse::new("Hi there!", usage);

        assert_eq!(response.usage, Some(usage));
    }

    #[test]
    fn chat_response_without_usage_represents_unavailable_usage() {
        let response = ChatResponse::without_usage("Hi there!");

        assert_eq!(response.usage, None);
    }
}
