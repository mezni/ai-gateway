use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;

use crate::domain::{
    ChatRequest, ChatResponse, Message, MessageRole, is_max_tokens_in_range,
    is_temperature_in_range,
};

const MOCK_COMPLETION_CONTENT: &str = "This is a mocked chat completion.";

// ---------------------------------------------------------------------------
// Provider boundary
//
// The trait lives in the application layer rather than beside its
// implementations because `specs/002-layered-architecture/contracts/layout.md`
// rule 4 forbids the application layer from importing `infrastructure`. Only
// the calling layer may own the contract; adapters in
// `src/infrastructure/providers/` implement it.
// ---------------------------------------------------------------------------

/// A provider call that did not yield a usable response.
///
/// Deliberately fieldless: a category names *what kind* of failure occurred and
/// carries no message, status code, URL, model name, or any other string. That
/// makes provider detail unrepresentable rather than merely filtered on the way
/// out, so no future adapter can leak it by forgetting to redact (FR-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFailure {
    /// The provider could not be contacted at all.
    Unreachable,
    /// The provider was contacted but refused to serve the request.
    Refused,
    /// The provider did not answer within the configured deadline.
    DeadlineExceeded,
    /// The provider answered with a well-formed envelope the gateway cannot use.
    UnusableResponse,
    /// The provider answered with something that is not a response at all.
    InvalidResponse,
}

/// The gateway's single outbound contract for chat completions.
///
/// Every provider — deterministic or otherwise — implements this. The signatures
/// mention only gateway-owned types (`ChatRequest`, `ChatResponse`), so a
/// provider-native type can never cross the boundary and a client can never
/// observe provider-specific structure in a response (FR-001, FR-002, FR-003).
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Produces a completion for a validated request.
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderFailure>;

    /// Stable identifier of this provider, used for selection and readiness.
    fn id(&self) -> &str;
}

/// Calls `provider.chat` under a deadline, so no provider can hang a request.
///
/// The bound is applied *here*, in the layer that owns the trait, rather than
/// inside each adapter. An adapter therefore cannot forget to apply it, and a
/// provider that never resolves becomes a `DeadlineExceeded` the gateway can
/// translate into a documented 504 (FR-011, research.md D-004).
pub async fn call_provider_bounded(
    provider: &dyn LlmProvider,
    request: &ChatRequest,
    deadline_ms: u64,
) -> Result<ChatResponse, ProviderFailure> {
    match timeout(Duration::from_millis(deadline_ms), provider.chat(request)).await {
        Ok(result) => result,
        Err(_elapsed) => Err(ProviderFailure::DeadlineExceeded),
    }
}

/// The documented rule an application-layer validation stage rejected.
///
/// Type-level failures — a control that is a string, boolean, `null`, object, or
/// array, a control supplied twice, a message that is not an object — never
/// reach this type. They fail during deserialization and are already normalized
/// to `invalid_request` by the `JsonRejection` mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationFailure {
    /// `model` was empty.
    InvalidModel,
    /// The message list was empty.
    InvalidMessages,
    /// A message role was not `system`, `user`, or `assistant`.
    InvalidRole,
    /// A message had empty content.
    InvalidContent,
    /// `temperature` was present but outside `0.0..=2.0`.
    TemperatureOutOfRange,
    /// `max_tokens` was present but outside `1..=4096`.
    MaxTokensOutOfRange,
    /// The request explicitly asked for a streamed response.
    StreamingRequested,
}

/// A rejected chat request, classified by the rule it violated.
///
/// The classification exists to make the ordered rule set testable; it is never
/// serialized to a client. `Display` yields the fixed public message only, so it
/// can never leak a field name, an input value, or a source detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationError {
    /// The rule that rejected the request.
    pub failure: ValidationFailure,
}

impl ValidationError {
    /// The message returned for every non-streaming validation failure.
    pub const INVALID_REQUEST: &'static str = "The chat request is invalid.";
    /// The message returned when a client asks for an unimplemented stream.
    pub const STREAMING_UNSUPPORTED: &'static str = "Streaming is not supported.";

    /// Creates a validation error for the given failure.
    pub fn new(failure: ValidationFailure) -> Self {
        Self { failure }
    }

    /// The exact client-facing message for this failure.
    pub fn message(&self) -> &'static str {
        match self.failure {
            ValidationFailure::StreamingRequested => Self::STREAMING_UNSUPPORTED,
            ValidationFailure::InvalidModel
            | ValidationFailure::InvalidMessages
            | ValidationFailure::InvalidRole
            | ValidationFailure::InvalidContent
            | ValidationFailure::TemperatureOutOfRange
            | ValidationFailure::MaxTokensOutOfRange => Self::INVALID_REQUEST,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteChatCommand {
    pub model: String,
    pub messages: Vec<IncomingMessage>,
    /// `None` means the client did not supply the control; no default is invented.
    pub temperature: Option<f64>,
    /// `None` means the client did not supply the control; no default is invented.
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct ValidatedChat {
    request: ChatRequest,
    stream_requested: bool,
}

impl ValidatedChat {
    pub fn model(&self) -> &str {
        &self.request.model
    }

    /// The validated request handed to the provider.
    pub fn request(&self) -> &ChatRequest {
        &self.request
    }

    /// The sampling temperature the client supplied, or `None` when it supplied
    /// none. No default is invented for an omitted control.
    pub fn temperature(&self) -> Option<f64> {
        self.request.temperature()
    }

    /// The response-length cap the client supplied, or `None` when it supplied
    /// none. No default is invented for an omitted control.
    pub fn max_tokens(&self) -> Option<u32> {
        self.request.max_tokens()
    }

    /// Whether the client explicitly asked for a streamed response. Once the
    /// stream stage is enforced, a validated request only ever records `false`.
    pub fn stream_requested(&self) -> bool {
        self.stream_requested
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCompletion {
    pub content: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MockChatCompletionService;

impl MockChatCompletionService {
    pub fn new() -> Self {
        Self
    }

    /// Convenience for callers that already hold a service instance.
    pub fn validate(&self, command: CompleteChatCommand) -> Result<ValidatedChat, ValidationError> {
        ChatValidator::validate(command)
    }

    /// The fixed completion this service returns, bypassing the provider trait.
    ///
    /// Retained as a direct accessor; request handling goes through
    /// [`LlmProvider::chat`] so every completion passes the provider boundary.
    pub fn complete(&self, _chat: ValidatedChat) -> ChatCompletion {
        let response = ChatResponse::without_usage(MOCK_COMPLETION_CONTENT);
        ChatCompletion {
            content: response.content,
        }
    }
}

/// Ordered validation of a raw chat command.
///
/// Validation is a separate concern from completion, so it is not reached
/// through [`LlmProvider`]. That matters for two reasons: a request must be
/// rejected before any provider is contacted, and the application layer cannot
/// hold a provider instance it was handed by `infrastructure`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChatValidator;

impl ChatValidator {
    /// Evaluates the documented rule catalog in order and returns the single
    /// validated value that application logic is allowed to consume.
    ///
    /// Stage order follows `contracts/validation-rules.md` §3. Structural
    /// readability and control typing are settled by the DTO before a command
    /// exists, so the stages that remain here are control-range validity, then
    /// streaming. The first failure wins and no later stage runs.
    pub fn validate(command: CompleteChatCommand) -> Result<ValidatedChat, ValidationError> {
        let model = Self::required_model(command.model)?;
        let messages = Self::required_messages(command.messages)?;
        Self::control_ranges(command.temperature, command.max_tokens)?;
        Self::streaming(command.stream)?;

        Ok(ValidatedChat {
            request: ChatRequest::with_controls(
                model,
                messages,
                command.temperature,
                command.max_tokens,
            ),
            stream_requested: command.stream,
        })
    }

    /// Stage: required-field validity. `model` must be a non-empty string and
    /// is preserved exactly, including surrounding whitespace.
    fn required_model(model: String) -> Result<String, ValidationError> {
        if model.is_empty() {
            return Err(ValidationError::new(ValidationFailure::InvalidModel));
        }
        Ok(model)
    }

    /// Stage: streaming. This is the last stage, so a request that is invalid
    /// for any earlier reason is reported as such rather than as an unsupported
    /// feature. Only an explicit `true` is a breach; an absent or `false`
    /// control is a normal request.
    fn streaming(stream: bool) -> Result<(), ValidationError> {
        if stream {
            return Err(ValidationError::new(ValidationFailure::StreamingRequested));
        }

        Ok(())
    }

    /// Stage: control-range validity. A control the client omitted is never a
    /// breach, so `None` passes through untouched. A supplied value must fall
    /// inside the inclusive documented interval; a value that is not a real
    /// number in that interval is a breach too.
    fn control_ranges(
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<(), ValidationError> {
        if temperature.is_some_and(|value| !is_temperature_in_range(value)) {
            return Err(ValidationError::new(
                ValidationFailure::TemperatureOutOfRange,
            ));
        }
        if max_tokens.is_some_and(|value| !is_max_tokens_in_range(value)) {
            return Err(ValidationError::new(ValidationFailure::MaxTokensOutOfRange));
        }

        Ok(())
    }

    /// Stage: required-field validity. The list must be non-empty, keep its
    /// submitted order, and every entry needs a supported role and non-empty
    /// content.
    fn required_messages(messages: Vec<IncomingMessage>) -> Result<Vec<Message>, ValidationError> {
        if messages.is_empty() {
            return Err(ValidationError::new(ValidationFailure::InvalidMessages));
        }

        let mut converted = Vec::with_capacity(messages.len());
        for incoming in messages {
            let role = match incoming.role.as_str() {
                "system" => MessageRole::System,
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                _ => return Err(ValidationError::new(ValidationFailure::InvalidRole)),
            };
            if incoming.content.is_empty() {
                return Err(ValidationError::new(ValidationFailure::InvalidContent));
            }
            converted.push(Message::new(role, incoming.content));
        }

        Ok(converted)
    }
}

#[async_trait]
impl LlmProvider for MockChatCompletionService {
    async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
        Ok(ChatResponse::without_usage(MOCK_COMPLETION_CONTENT))
    }

    fn id(&self) -> &str {
        DETERMINISTIC_PROVIDER_ID
    }
}

/// Identifier of the built-in deterministic provider.
///
/// The Phase 3 `MockChatCompletionService` is already the deterministic
/// behaviour — a fixed completion that ignores the request — and it already
/// lived in this module. Implementing [`LlmProvider`] for it is therefore the
/// only way to give `AppState::new` a default provider without the application
/// layer importing `infrastructure` (layout rule 4), and without editing the
/// existing tests that assert this exact response body.
///
/// The production adapter in `src/infrastructure/providers/deterministic.rs`
/// serves the same content under the same id, so swapping one for the other
/// cannot change a single byte a client observes.
pub const DETERMINISTIC_PROVIDER_ID: &str = "deterministic";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ChatRequest, Message, MessageRole};

    fn command(model: &str, messages: &[(&str, &str)]) -> CompleteChatCommand {
        command_with(model, messages, None, None, false)
    }

    fn command_with_controls(
        model: &str,
        messages: &[(&str, &str)],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> CompleteChatCommand {
        command_with(model, messages, temperature, max_tokens, false)
    }

    fn command_with(
        model: &str,
        messages: &[(&str, &str)],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
        stream: bool,
    ) -> CompleteChatCommand {
        CompleteChatCommand {
            model: model.to_owned(),
            messages: messages
                .iter()
                .map(|(role, content)| IncomingMessage {
                    role: (*role).to_owned(),
                    content: (*content).to_owned(),
                })
                .collect(),
            temperature,
            max_tokens,
            stream,
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
    fn each_failure_classification_is_reported_for_its_own_rule() {
        let cases = [
            (
                ValidationFailure::InvalidModel,
                command("", &[("user", "Hello!")]),
            ),
            (
                ValidationFailure::InvalidMessages,
                command("mock-model", &[]),
            ),
            (
                ValidationFailure::InvalidRole,
                command("mock-model", &[("tool", "Hello!")]),
            ),
            (
                ValidationFailure::InvalidContent,
                command("mock-model", &[("user", "")]),
            ),
        ];

        for (expected, command) in cases {
            let error = service().validate(command).unwrap_err();

            assert_eq!(error.failure, expected);
        }
    }

    #[test]
    fn every_non_stream_failure_renders_the_single_fixed_invalid_message() {
        for failure in [
            ValidationFailure::InvalidModel,
            ValidationFailure::InvalidMessages,
            ValidationFailure::InvalidRole,
            ValidationFailure::InvalidContent,
            ValidationFailure::TemperatureOutOfRange,
            ValidationFailure::MaxTokensOutOfRange,
        ] {
            let error = ValidationError::new(failure);

            assert_eq!(error.message(), "The chat request is invalid.");
            assert_eq!(error.to_string(), "The chat request is invalid.");
        }
    }

    #[test]
    fn stream_failure_renders_the_documented_streaming_message() {
        let error = ValidationError::new(ValidationFailure::StreamingRequested);

        assert_eq!(error.message(), "Streaming is not supported.");
        assert_eq!(error.to_string(), "Streaming is not supported.");
    }

    #[test]
    fn validation_error_display_is_always_one_of_the_two_documented_messages() {
        for failure in [
            ValidationFailure::InvalidModel,
            ValidationFailure::InvalidMessages,
            ValidationFailure::InvalidRole,
            ValidationFailure::InvalidContent,
            ValidationFailure::TemperatureOutOfRange,
            ValidationFailure::MaxTokensOutOfRange,
            ValidationFailure::StreamingRequested,
        ] {
            let error = ValidationError::new(failure);

            assert!(
                matches!(
                    error.to_string().as_str(),
                    "The chat request is invalid." | "Streaming is not supported."
                ),
                "undocumented message for {failure:?}: {:?}",
                error.to_string()
            );
        }
    }

    #[test]
    fn invalid_request_display_never_contains_a_field_or_rule_name() {
        let forbidden = [
            "model",
            "messages",
            "role",
            "content",
            "temperature",
            "max_tokens",
            "stream",
            "Invalid",
            "OutOfRange",
            "Streaming",
            "validation",
        ];

        for failure in [
            ValidationFailure::InvalidModel,
            ValidationFailure::InvalidMessages,
            ValidationFailure::InvalidRole,
            ValidationFailure::InvalidContent,
            ValidationFailure::TemperatureOutOfRange,
            ValidationFailure::MaxTokensOutOfRange,
        ] {
            let message = ValidationError::new(failure).to_string();

            for needle in forbidden {
                assert!(
                    !message.contains(needle),
                    "leaked {needle:?} in {message:?}"
                );
            }
        }
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
    fn control_range_stage_accepts_inclusive_temperature_boundaries() {
        for temperature in [Some(0.0), Some(1.0), Some(2.0), None] {
            let validated = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    temperature,
                    Some(1),
                ))
                .unwrap();

            assert_eq!(validated.temperature(), temperature);
        }
    }

    #[test]
    fn control_range_stage_accepts_inclusive_max_tokens_boundaries() {
        for max_tokens in [Some(1), Some(500), Some(4096), None] {
            let validated = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    Some(1.0),
                    max_tokens,
                ))
                .unwrap();

            assert_eq!(validated.max_tokens(), max_tokens);
        }
    }

    #[test]
    fn control_range_stage_rejects_temperature_outside_the_interval() {
        for temperature in [-0.1, -1.0, 2.000_1, 3.0, 1e9] {
            let error = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    Some(temperature),
                    None,
                ))
                .unwrap_err();

            assert_eq!(error.failure, ValidationFailure::TemperatureOutOfRange);
            assert_eq!(error.to_string(), "The chat request is invalid.");
        }
    }

    #[test]
    fn control_range_stage_rejects_max_tokens_outside_the_interval() {
        for max_tokens in [0, 4097, 8192, u32::MAX] {
            let error = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    None,
                    Some(max_tokens),
                ))
                .unwrap_err();

            assert_eq!(error.failure, ValidationFailure::MaxTokensOutOfRange);
            assert_eq!(error.to_string(), "The chat request is invalid.");
        }
    }

    #[test]
    fn control_range_stage_rejects_each_control_independently() {
        let bad_temperature = service()
            .validate(command_with_controls(
                "mock-model",
                &[("user", "Hello!")],
                Some(9.0),
                Some(500),
            ))
            .unwrap_err();
        let bad_max_tokens = service()
            .validate(command_with_controls(
                "mock-model",
                &[("user", "Hello!")],
                Some(0.5),
                Some(0),
            ))
            .unwrap_err();

        assert_eq!(
            bad_temperature.failure,
            ValidationFailure::TemperatureOutOfRange
        );
        assert_eq!(
            bad_max_tokens.failure,
            ValidationFailure::MaxTokensOutOfRange
        );
    }

    #[test]
    fn required_field_failure_outranks_control_range_failure() {
        let error = service()
            .validate(command_with_controls(
                "",
                &[("user", "Hello!")],
                Some(9.0),
                Some(0),
            ))
            .unwrap_err();

        assert_eq!(error.failure, ValidationFailure::InvalidModel);
    }

    #[test]
    fn role_failure_outranks_control_range_failure() {
        let error = service()
            .validate(command_with_controls(
                "mock-model",
                &[("tool", "Hello!")],
                Some(9.0),
                Some(0),
            ))
            .unwrap_err();

        assert_eq!(error.failure, ValidationFailure::InvalidRole);
    }

    #[test]
    fn control_range_stage_rejects_controls_that_are_not_real_numbers() {
        for temperature in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    Some(temperature),
                    None,
                ))
                .unwrap_err();

            assert_eq!(error.failure, ValidationFailure::TemperatureOutOfRange);
        }
    }

    #[test]
    fn validated_chat_carries_exactly_the_supplied_control_values() {
        let validated = service()
            .validate(command_with_controls(
                "mock-model",
                &[("user", "Hello!")],
                Some(0.7),
                Some(500),
            ))
            .unwrap();

        assert_eq!(validated.temperature(), Some(0.7));
        assert_eq!(validated.max_tokens(), Some(500));
    }

    #[test]
    fn validated_chat_records_omitted_controls_as_unspecified() {
        let validated = service()
            .validate(command("mock-model", &[("user", "Hello!")]))
            .unwrap();

        assert_eq!(validated.temperature(), None);
        assert_eq!(validated.max_tokens(), None);
    }

    #[test]
    fn validated_chat_carries_each_control_independently() {
        let cases = [(Some(0.0), None), (None, Some(1)), (Some(2.0), Some(4096))];

        for (temperature, max_tokens) in cases {
            let validated = service()
                .validate(command_with_controls(
                    "mock-model",
                    &[("user", "Hello!")],
                    temperature,
                    max_tokens,
                ))
                .unwrap();

            assert_eq!(validated.temperature(), temperature);
            assert_eq!(validated.max_tokens(), max_tokens);
        }
    }

    #[test]
    fn controls_do_not_change_the_underlying_domain_request_fidelity() {
        let model = "  MiXeD/Mock\t\u{1f642}  ";
        let content = "  You are helpful.  ";

        let validated = service()
            .validate(command_with_controls(
                model,
                &[("system", content)],
                Some(0.5),
                Some(64),
            ))
            .unwrap();

        assert_eq!(validated.model(), model);
        assert_eq!(valid(&validated).messages[0].content, content);
        assert_eq!(valid(&validated).temperature, Some(0.5));
    }

    #[test]
    fn the_stream_stage_refuses_an_explicit_stream_request() {
        let error = service()
            .validate(command_with(
                "mock-model",
                &[("user", "Hello!")],
                None,
                None,
                true,
            ))
            .unwrap_err();

        assert_eq!(error.failure, ValidationFailure::StreamingRequested);
        assert_eq!(error.to_string(), "Streaming is not supported.");
    }

    #[test]
    fn a_false_stream_request_validates_and_records_no_stream() {
        let validated = service()
            .validate(command_with(
                "mock-model",
                &[("user", "Hello!")],
                None,
                None,
                false,
            ))
            .unwrap();

        assert!(!validated.stream_requested());
    }

    #[test]
    fn an_absent_stream_control_validates_and_records_no_stream() {
        let validated = service()
            .validate(command("mock-model", &[("user", "Hello!")]))
            .unwrap();

        assert!(!validated.stream_requested());
    }

    #[test]
    fn the_stream_stage_runs_after_every_other_stage() {
        // Each case breaks an earlier stage and also asks for a stream; the
        // earlier stage's failure is the one reported.
        let cases = [
            (
                command_with("", &[("user", "Hello!")], None, None, true),
                ValidationFailure::InvalidModel,
            ),
            (
                command_with("mock-model", &[], None, None, true),
                ValidationFailure::InvalidMessages,
            ),
            (
                command_with("mock-model", &[("tool", "Hello!")], None, None, true),
                ValidationFailure::InvalidRole,
            ),
            (
                command_with("mock-model", &[("user", "")], None, None, true),
                ValidationFailure::InvalidContent,
            ),
            (
                command_with("mock-model", &[("user", "Hello!")], Some(9.0), None, true),
                ValidationFailure::TemperatureOutOfRange,
            ),
            (
                command_with("mock-model", &[("user", "Hello!")], None, Some(0), true),
                ValidationFailure::MaxTokensOutOfRange,
            ),
        ];

        for (command, expected) in cases {
            let error = service().validate(command).unwrap_err();

            assert_eq!(error.failure, expected);
        }
    }

    #[test]
    fn a_stream_request_is_refused_after_valid_controls_are_accepted() {
        // In-range controls must not rescue a stream request: the stage that
        // fails is still the stream stage.
        let error = service()
            .validate(command_with(
                "mock-model",
                &[("user", "Hello!")],
                Some(1.0),
                Some(500),
                true,
            ))
            .unwrap_err();

        assert_eq!(error.failure, ValidationFailure::StreamingRequested);
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

    // -----------------------------------------------------------------------
    // Provider boundary
    // -----------------------------------------------------------------------

    /// A provider that never answers, so only the gateway's own bound can end
    /// the call.
    struct HangingProvider;

    #[async_trait::async_trait]
    impl LlmProvider for HangingProvider {
        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
            std::future::pending().await
        }

        fn id(&self) -> &str {
            "hanging"
        }
    }

    /// A provider that reports a category, to prove the boundary passes it
    /// through unchanged.
    struct FailingProvider(ProviderFailure);

    #[async_trait::async_trait]
    impl LlmProvider for FailingProvider {
        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
            Err(self.0)
        }

        fn id(&self) -> &str {
            "failing"
        }
    }

    fn valid_request() -> ChatRequest {
        ChatRequest::new(
            "mock-model",
            vec![Message::new(MessageRole::User, "Hello!")],
        )
    }

    #[tokio::test]
    async fn bounded_call_turns_a_never_resolving_provider_into_deadline_exceeded() {
        // Without the bound this test would hang rather than fail, which is
        // exactly the regression it guards.
        let started = std::time::Instant::now();

        let result = call_provider_bounded(&HangingProvider, &valid_request(), 50).await;

        assert_eq!(result.unwrap_err(), ProviderFailure::DeadlineExceeded);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "the bound must fire well before the test timeout"
        );
    }

    #[tokio::test]
    async fn bounded_call_returns_the_provider_response_when_it_arrives_in_time() {
        let provider = MockChatCompletionService::new();

        let response = call_provider_bounded(&provider, &valid_request(), 5_000)
            .await
            .unwrap();

        assert_eq!(response.content, MOCK_COMPLETION_CONTENT);
        assert!(response.usage.is_none());
    }

    #[tokio::test]
    async fn bounded_call_does_not_rewrite_a_provider_failure() {
        for failure in [
            ProviderFailure::Unreachable,
            ProviderFailure::Refused,
            ProviderFailure::UnusableResponse,
            ProviderFailure::InvalidResponse,
        ] {
            let result = call_provider_bounded(&FailingProvider(failure), &valid_request(), 5_000)
                .await;

            assert_eq!(result.unwrap_err(), failure);
        }
    }

    #[tokio::test]
    async fn default_provider_reports_the_documented_id() {
        assert_eq!(MockChatCompletionService::new().id(), "deterministic");
        assert_eq!(MockChatCompletionService::new().id(), DETERMINISTIC_PROVIDER_ID);
    }

    #[test]
    fn provider_failure_carries_no_data() {
        // A fieldless enum cannot hold a message, URL, or model name, so no
        // adapter can leak provider detail through it (FR-010).
        assert_eq!(
            std::mem::size_of::<ProviderFailure>(),
            std::mem::size_of::<u8>()
        );
    }
}
