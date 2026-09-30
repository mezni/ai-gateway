//! The built-in deterministic provider.
//!
//! Serves a fixed completion without contacting anything, so the gateway runs
//! with no configuration and no network (FR-006, constitution principle IX).

use async_trait::async_trait;

use crate::application::chat::{LlmProvider, ProviderFailure};
use crate::domain::{ChatRequest, ChatResponse};

/// The completion this provider returns for every request.
///
/// Matches the Phase 3 response body exactly, so selecting this adapter through
/// the registry is byte-for-byte indistinguishable from the default
/// `AppState::new` path.
pub const DETERMINISTIC_CONTENT: &str = "This is a mocked chat completion.";

/// Identifier operators select this provider by.
pub const DETERMINISTIC_ID: &str = "deterministic";

/// A provider whose answer depends on nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicProvider;

impl DeterministicProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LlmProvider for DeterministicProvider {
    async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
        Ok(ChatResponse::without_usage(DETERMINISTIC_CONTENT))
    }

    fn id(&self) -> &str {
        DETERMINISTIC_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Message, MessageRole};

    fn request(model: &str, content: &str) -> ChatRequest {
        ChatRequest::new(model, vec![Message::new(MessageRole::User, content)])
    }

    #[tokio::test]
    async fn returns_the_fixed_completion() {
        let response = DeterministicProvider::new()
            .chat(&request("mock-model", "Hello!"))
            .await
            .unwrap();

        assert_eq!(response.content, DETERMINISTIC_CONTENT);
    }

    #[tokio::test]
    async fn reports_no_usage_rather_than_inventing_token_counts() {
        let response = DeterministicProvider::new()
            .chat(&request("mock-model", "Hello!"))
            .await
            .unwrap();

        assert!(response.usage.is_none());
    }

    #[tokio::test]
    async fn is_deterministic_across_different_requests() {
        let provider = DeterministicProvider::new();

        let first = provider
            .chat(&request("model-a", "one"))
            .await
            .unwrap();
        let second = provider
            .chat(&request("model-b", "a completely different question"))
            .await
            .unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn is_deterministic_across_repeated_calls() {
        let provider = DeterministicProvider::new();
        let request = request("mock-model", "Hello!");

        let first = provider.chat(&request).await.unwrap();
        let second = provider.chat(&request).await.unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn reports_the_documented_id() {
        assert_eq!(DeterministicProvider::new().id(), "deterministic");
    }
}
