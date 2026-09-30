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

/// Smallest accepted `temperature`, inclusive.
pub const MIN_TEMPERATURE: f64 = 0.0;

/// Largest accepted `temperature`, inclusive.
pub const MAX_TEMPERATURE: f64 = 2.0;

/// Smallest accepted `max_tokens`, inclusive.
pub const MIN_MAX_TOKENS: u32 = 1;

/// Largest accepted `max_tokens`, inclusive.
pub const MAX_MAX_TOKENS: u32 = 4096;

/// Reports whether `value` is an accepted `temperature`.
///
/// The bounds are inclusive, so `0.0` and `2.0` are both accepted.
pub fn is_temperature_in_range(value: f64) -> bool {
    (MIN_TEMPERATURE..=MAX_TEMPERATURE).contains(&value)
}

/// Reports whether `value` is an accepted `max_tokens`.
///
/// The bounds are inclusive, so `1` and `4096` are both accepted.
pub fn is_max_tokens_in_range(value: u32) -> bool {
    (MIN_MAX_TOKENS..=MAX_MAX_TOKENS).contains(&value)
}

/// A client's request for a chat completion.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    /// Model reference the request targets.
    pub model: String,
    /// Messages to send, in conversation order.
    pub messages: Vec<Message>,
    /// Sampling temperature the client supplied, or `None` when it supplied none.
    ///
    /// `None` means "unspecified" and is never replaced by an invented default,
    /// so an absent control is never reported as a client-supplied value.
    pub temperature: Option<f64>,
    /// Response-length cap the client supplied, or `None` when it supplied none.
    pub max_tokens: Option<u32>,
}

impl ChatRequest {
    /// Creates a chat request for the given model and messages, with neither
    /// generation control specified.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Creates a chat request carrying the generation controls the client
    /// supplied. `None` records that the client supplied nothing.
    pub fn with_controls(
        model: impl Into<String>,
        messages: Vec<Message>,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature,
            max_tokens,
        }
    }

    /// The sampling temperature, or `None` when the client specified none.
    pub fn temperature(&self) -> Option<f64> {
        self.temperature
    }

    /// The response-length cap, or `None` when the client specified none.
    pub fn max_tokens(&self) -> Option<u32> {
        self.max_tokens
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
    fn control_bounds_hold_the_documented_inclusive_values() {
        assert_eq!(MIN_TEMPERATURE, 0.0);
        assert_eq!(MAX_TEMPERATURE, 2.0);
        assert_eq!(MIN_MAX_TOKENS, 1);
        assert_eq!(MAX_MAX_TOKENS, 4096);
    }

    #[test]
    fn is_temperature_in_range_accepts_only_the_inclusive_interval() {
        for accepted in [0.0, 0.5, 1.0, 1.999_9, 2.0] {
            assert!(
                is_temperature_in_range(accepted),
                "{accepted} should be accepted"
            );
        }

        for rejected in [-1.0, -0.1, -0.000_1, 2.000_1, 3.0, 1e9] {
            assert!(
                !is_temperature_in_range(rejected),
                "{rejected} should be rejected"
            );
        }
    }

    #[test]
    fn is_temperature_in_range_rejects_values_that_are_not_numbers() {
        assert!(!is_temperature_in_range(f64::NAN));
        assert!(!is_temperature_in_range(f64::INFINITY));
        assert!(!is_temperature_in_range(f64::NEG_INFINITY));
    }

    #[test]
    fn is_max_tokens_in_range_accepts_only_the_inclusive_interval() {
        for accepted in [1, 2, 5, 500, 4095, 4096] {
            assert!(
                is_max_tokens_in_range(accepted),
                "{accepted} should be accepted"
            );
        }

        for rejected in [0, 4097, 8192, u32::MAX] {
            assert!(
                !is_max_tokens_in_range(rejected),
                "{rejected} should be rejected"
            );
        }
    }

    #[test]
    fn message_role_variants_cover_system_user_assistant() {
        assert_eq!(format!("{}", MessageRole::System), "system");
        assert_eq!(format!("{}", MessageRole::User), "user");
        assert_eq!(format!("{}", MessageRole::Assistant), "assistant");
    }

    #[test]
    fn chat_request_new_records_both_controls_as_unspecified() {
        let request = ChatRequest::new("test-model", vec![Message::new(MessageRole::User, "Hi")]);

        assert_eq!(request.temperature, None);
        assert_eq!(request.max_tokens, None);
        assert_eq!(request.temperature(), None);
        assert_eq!(request.max_tokens(), None);
    }

    #[test]
    fn chat_request_new_equals_with_controls_given_no_values() {
        let messages = vec![Message::new(MessageRole::User, "Hi")];

        assert_eq!(
            ChatRequest::new("test-model", messages.clone()),
            ChatRequest::with_controls("test-model", messages, None, None)
        );
    }

    #[test]
    fn with_controls_retains_exactly_the_supplied_values() {
        let request = ChatRequest::with_controls(
            "test-model",
            vec![Message::new(MessageRole::User, "Hi")],
            Some(0.7),
            Some(500),
        );

        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(500));
        assert_eq!(request.temperature(), Some(0.7));
        assert_eq!(request.max_tokens(), Some(500));
    }

    #[test]
    fn with_controls_retains_boundary_and_unspecified_combinations() {
        let messages = vec![Message::new(MessageRole::User, "Hi")];
        let cases = [
            (Some(0.0), None),
            (Some(2.0), None),
            (None, Some(1)),
            (None, Some(4096)),
            (Some(1.25), Some(1)),
        ];

        for (temperature, max_tokens) in cases {
            let request =
                ChatRequest::with_controls("test-model", messages.clone(), temperature, max_tokens);

            assert_eq!(request.temperature(), temperature);
            assert_eq!(request.max_tokens(), max_tokens);
        }
    }

    #[test]
    fn controls_do_not_disturb_model_or_message_fidelity() {
        let model = "  MiXeD/Mock\t\u{1f642}  ";
        let content = "  line one\twith  internal   spaces \n  line two  ";

        let request = ChatRequest::with_controls(
            model,
            vec![Message::new(MessageRole::User, content)],
            Some(0.5),
            Some(64),
        );

        assert_eq!(request.model, model);
        assert_eq!(request.messages[0].content, content);
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
