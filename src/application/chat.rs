use thiserror::Error;

use crate::domain::{ChatRequest, ChatResponse, Message, MessageRole};

const MOCK_COMPLETION_CONTENT: &str = "This is a mocked chat completion.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteChatCommand {
    pub model: String,
    pub messages: Vec<IncomingMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct ValidatedChat {
    request: ChatRequest,
}

impl ValidatedChat {
    pub fn model(&self) -> &str {
        &self.request.model
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCompletion {
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("The chat request is invalid.")]
pub struct InvalidChatRequest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MockChatCompletionService;

impl MockChatCompletionService {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(
        &self,
        command: CompleteChatCommand,
    ) -> Result<ValidatedChat, InvalidChatRequest> {
        if command.model.is_empty() || command.messages.is_empty() {
            return Err(InvalidChatRequest);
        }

        let mut messages = Vec::with_capacity(command.messages.len());
        for incoming in command.messages {
            let role = match incoming.role.as_str() {
                "system" => MessageRole::System,
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                _ => return Err(InvalidChatRequest),
            };
            if incoming.content.is_empty() {
                return Err(InvalidChatRequest);
            }
            messages.push(Message::new(role, incoming.content));
        }

        Ok(ValidatedChat {
            request: ChatRequest::new(command.model, messages),
        })
    }

    pub fn complete(&self, _chat: ValidatedChat) -> ChatCompletion {
        let response = ChatResponse::without_usage(MOCK_COMPLETION_CONTENT);
        ChatCompletion {
            content: response.content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ChatRequest, Message, MessageRole};

    fn command(model: &str, messages: &[(&str, &str)]) -> CompleteChatCommand {
        CompleteChatCommand {
            model: model.to_owned(),
            messages: messages
                .iter()
                .map(|(role, content)| IncomingMessage {
                    role: (*role).to_owned(),
                    content: (*content).to_owned(),
                })
                .collect(),
        }
    }

    fn service() -> MockChatCompletionService {
        MockChatCompletionService::new()
    }

    fn valid(validated: &ValidatedChat) -> &ChatRequest {
        &validated.request
    }

    #[test]
    fn valid_command_maps_to_ordered_domain_chat_request() {
        let model = "  MiXeD/Mock\t\u{1f642}  ";
        let system = "  You are helpful.  ";
        let user = "\u{1f680} Hello\u{1f680}";
        let assistant = "  Hi  there!  ";

        let validated = service()
            .validate(command(
                model,
                &[("system", system), ("user", user), ("assistant", assistant)],
            ))
            .unwrap();

        assert_eq!(
            valid(&validated),
            &ChatRequest::new(
                model,
                vec![
                    Message::new(MessageRole::System, system),
                    Message::new(MessageRole::User, user),
                    Message::new(MessageRole::Assistant, assistant),
                ],
            )
        );
    }

    #[test]
    fn validate_rejects_empty_model() {
        let result = service().validate(command("", &[("user", "Hello!")]));

        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_whitespace_only_model_without_trimming() {
        let model = " \t\u{1f642}  ";

        let validated = service()
            .validate(command(model, &[("user", "Hello!")]))
            .unwrap();

        assert_eq!(valid(&validated).model, model);
    }

    #[test]
    fn validate_accepts_unregistered_noncanonical_model_without_lookup() {
        let model = "totally/unregistered-Model \u{1f642} v9";

        let validated = service()
            .validate(command(model, &[("user", "Hello!")]))
            .unwrap();

        assert_eq!(valid(&validated).model, model);
    }

    #[test]
    fn validate_rejects_empty_message_list() {
        let result = service().validate(command("mock-model", &[]));

        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_single_message_boundary() {
        let validated = service()
            .validate(command("mock-model", &[("user", "Hello!")]))
            .unwrap();

        assert_eq!(
            valid(&validated),
            &ChatRequest::new(
                "mock-model",
                vec![Message::new(MessageRole::User, "Hello!")]
            )
        );
    }

    #[test]
    fn validate_accepts_system_role() {
        let validated = service()
            .validate(command("mock-model", &[("system", "You are helpful.")]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].role, MessageRole::System);
    }

    #[test]
    fn validate_accepts_user_role() {
        let validated = service()
            .validate(command("mock-model", &[("user", "Hello!")]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].role, MessageRole::User);
    }

    #[test]
    fn validate_accepts_assistant_role() {
        let validated = service()
            .validate(command("mock-model", &[("assistant", "Hi there!")]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].role, MessageRole::Assistant);
    }

    #[test]
    fn validate_rejects_unsupported_roles() {
        let unsupported = [
            "",
            "tool",
            "developer",
            "function",
            " user ",
            "User",
            "USER",
            "system\n",
        ];

        for role in unsupported {
            let result = service().validate(command("mock-model", &[(role, "Hello!")]));

            assert!(result.is_err(), "expected role {role:?} to be rejected");
        }
    }

    #[test]
    fn validate_checks_roles_in_later_messages() {
        let result = service().validate(command(
            "mock-model",
            &[("user", "Hello!"), ("tool", "calling a tool")],
        ));

        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_empty_content_in_first_message() {
        let result = service().validate(command("mock-model", &[("user", "")]));

        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_empty_content_in_a_later_message() {
        let result = service().validate(command(
            "mock-model",
            &[("user", "Hello!"), ("assistant", "")],
        ));

        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_whitespace_only_content() {
        let content = " \t\u{1f642} ";

        let validated = service()
            .validate(command("mock-model", &[("user", content)]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].content, content);
    }

    #[test]
    fn validate_preserves_leading_trailing_and_internal_whitespace() {
        let content = "  line one\twith  internal   spaces \n  line two  ";

        let validated = service()
            .validate(command("mock-model", &[("user", content)]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].content, content);
    }

    #[test]
    fn validate_preserves_unicode_and_control_characters() {
        let content = "\u{1f680} first line\n\u{4f60}\u{597d} second \u{1f642}";

        let validated = service()
            .validate(command("mock-model", &[("user", content)]))
            .unwrap();

        assert_eq!(valid(&validated).messages[0].content, content);
    }

    #[test]
    fn validate_allows_assistant_only_duplicate_and_mixed_role_sequences() {
        let assistant_only = "Still only assistant messages.";

        let validated = service()
            .validate(command(
                "mock-model",
                &[
                    ("assistant", "Only assistant messages."),
                    ("assistant", assistant_only),
                    ("user", "A user turn."),
                    ("user", "A user turn."),
                    ("system", "Instructions."),
                    ("assistant", "A reply."),
                ],
            ))
            .unwrap();

        assert_eq!(
            valid(&validated),
            &ChatRequest::new(
                "mock-model",
                vec![
                    Message::new(MessageRole::Assistant, "Only assistant messages."),
                    Message::new(MessageRole::Assistant, assistant_only),
                    Message::new(MessageRole::User, "A user turn."),
                    Message::new(MessageRole::User, "A user turn."),
                    Message::new(MessageRole::System, "Instructions."),
                    Message::new(MessageRole::Assistant, "A reply."),
                ],
            )
        );
    }

    #[test]
    fn validate_applies_no_message_count_or_content_size_cap() {
        let count = 32usize;
        let content = "x".repeat(50_000);
        let messages: Vec<(&str, &str)> = (0..count)
            .map(|index| {
                let role = if index % 2 == 0 { "user" } else { "assistant" };
                (role, content.as_str())
            })
            .collect();

        let validated = service()
            .validate(command("mock-model", &messages))
            .unwrap();

        assert_eq!(valid(&validated).messages.len(), count);
        assert_eq!(valid(&validated).messages[0].content, content);
        assert_eq!(
            valid(&validated).messages[count - 1].role,
            MessageRole::Assistant
        );
    }

    #[test]
    fn all_minimum_validation_failures_return_the_same_fixed_error() {
        let empty_model = service()
            .validate(command("", &[("user", "Hello!")]))
            .unwrap_err();
        let empty_messages = service().validate(command("mock-model", &[])).unwrap_err();
        let bad_role = service()
            .validate(command("mock-model", &[("tool", "Hello!")]))
            .unwrap_err();
        let empty_content = service()
            .validate(command("mock-model", &[("user", "")]))
            .unwrap_err();

        assert_eq!(empty_model.to_string(), "The chat request is invalid.");
        assert_eq!(empty_messages.to_string(), "The chat request is invalid.");
        assert_eq!(bad_role.to_string(), "The chat request is invalid.");
        assert_eq!(empty_content.to_string(), "The chat request is invalid.");
    }

    #[test]
    fn validation_error_does_not_echo_offending_values() {
        let sentinels = [
            "SENTINEL_MODEL_XYZ",
            "SENTINEL_ROLE_XYZ",
            "SENTINEL_PROMPT_XYZ",
        ];

        let empty_model = service()
            .validate(command("", &[("user", sentinels[2])]))
            .unwrap_err();
        let bad_role = service()
            .validate(command(sentinels[0], &[(sentinels[1], sentinels[2])]))
            .unwrap_err();
        let empty_content = service()
            .validate(command(sentinels[0], &[("user", "")]))
            .unwrap_err()
            .to_string();

        for message in [empty_model.to_string(), bad_role.to_string(), empty_content] {
            assert_eq!(message, "The chat request is invalid.");
            for sentinel in sentinels {
                assert!(!message.contains(sentinel));
            }
        }
    }

    #[test]
    fn complete_returns_exact_fixed_mock_content() {
        let completion = service().complete(
            service()
                .validate(command("mock-model", &[("user", "Hello!")]))
                .unwrap(),
        );

        assert_eq!(
            completion,
            ChatCompletion {
                content: "This is a mocked chat completion.".to_owned(),
            }
        );
    }

    #[test]
    fn complete_is_deterministic_for_repeated_valid_requests() {
        let mock = service();

        let first = mock
            .complete(
                mock.validate(command("mock-model", &[("user", "Hello!")]))
                    .unwrap(),
            )
            .content;
        let second = mock
            .complete(
                mock.validate(command("mock-model", &[("user", "Hello!")]))
                    .unwrap(),
            )
            .content;
        let third = mock
            .complete(
                mock.validate(command("mock-model", &[("user", "Hello!")]))
                    .unwrap(),
            )
            .content;

        assert_eq!(first, "This is a mocked chat completion.");
        assert_eq!(second, "This is a mocked chat completion.");
        assert_eq!(third, "This is a mocked chat completion.");
        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn complete_ignores_different_models_and_message_content() {
        let mock = service();

        let alpha = mock
            .complete(
                mock.validate(command(
                    "alpha/model",
                    &[("system", "Be terse."), ("user", "ALPHA_PROMPT_MARKER")],
                ))
                .unwrap(),
            )
            .content;
        let beta = mock
            .complete(
                mock.validate(command("beta-model", &[("user", "BETA_PROMPT_MARKER")]))
                    .unwrap(),
            )
            .content;

        assert_eq!(alpha, "This is a mocked chat completion.");
        assert_eq!(beta, "This is a mocked chat completion.");
        assert!(!alpha.contains("alpha/model"));
        assert!(!alpha.contains("ALPHA_PROMPT_MARKER"));
        assert!(!beta.contains("beta-model"));
        assert!(!beta.contains("BETA_PROMPT_MARKER"));
    }

    #[test]
    fn stateless_service_can_be_reused_without_cross_request_state() {
        let mock = service();

        let cases = [
            ("first/model", vec![("user", "First prompt.")]),
            (
                "second/model",
                vec![("user", "Second prompt."), ("assistant", "Reply.")],
            ),
            (
                "third/model",
                vec![("system", "Instructions."), ("user", "Third prompt.")],
            ),
            ("first/model", vec![("user", "First prompt.")]),
        ];

        for (model, messages) in cases {
            let completion = mock.complete(mock.validate(command(model, &messages)).unwrap());

            assert_eq!(completion.content, "This is a mocked chat completion.");
        }
    }
}
