//! The OpenRouter provider adapter.
//!
//! Maps the gateway's validated [`ChatRequest`] onto OpenRouter's
//! OpenAI-compatible chat-completion format, sends it to the configured
//! endpoint with a Bearer credential, and maps the response back to the
//! gateway's [`ChatResponse`]. Every failure is classified into one of the
//! five [`ProviderFailure`] categories; no provider status code, message text,
//! URL, or credential ever crosses the adapter boundary (Phase 4 provider
//! contract FR-010, FR-012, invariant 3).
//!
//! This adapter implements no timeout of its own: the application layer bounds
//! every call with the startup-validated deadline (FR-011). The client-level
//! bounds below are transport hygiene, not the policy deadline.

use std::env;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use thiserror::Error;

use crate::application::chat::{LlmProvider, ProviderFailure};
use crate::config::OpenRouterConfig;
use crate::domain::{ChatRequest, ChatResponse, Usage};

/// Identifier operators select this provider by.
pub const OPENROUTER_ID: &str = "openrouter";

/// Chat-completion path appended to the configured base URL.
pub const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

/// Outer bound on one API call: allow slow model responses without letting a
/// stalled connection pin a client request (research.md, finding 6).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Bound on establishing a connection: OpenRouter is a live service, so a
/// connection that is not up quickly is not going to become up.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// The raw API key, wrapped so a future `Debug` print can never render it.
#[derive(Clone)]
struct ApiKey(String);

impl ApiKey {
    fn new(raw: String) -> Self {
        Self(raw)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The OpenRouter adapter.
///
/// One instance is built at startup and shared by every request behind an
/// `Arc<dyn LlmProvider>` (registry rule: resolution returns the same shared
/// instance), so the HTTP client and its pool are not rebuilt per request.
#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<ApiKey>,
}

impl OpenRouterProvider {
    /// Constructs the adapter from provider configuration, reading the
    /// credential from the environment variable the configuration names.
    ///
    /// An unset or empty variable does not fail startup: the provider is still
    /// registered and reported, and refuses every chat request until a
    /// credential is present, so the operator sees the provider's effect on
    /// readiness rather than a silent startup crash.
    pub fn from_config(config: &OpenRouterConfig) -> Result<Self, OpenRouterProviderError> {
        let key = match env::var(&config.api_key_env) {
            Ok(value) if !value.trim().is_empty() => Some(value),
            Ok(_) | Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => None,
        };
        Self::new(&config.base_url, key)
    }

    /// Constructs the adapter from explicit values, for callers that already
    /// hold the credential (and for tests that must not touch the process
    /// environment).
    pub fn new(base_url: &str, api_key: Option<String>) -> Result<Self, OpenRouterProviderError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|error| OpenRouterProviderError::ClientConstruction {
                detail: error.to_string(),
            })?;
        let endpoint = format!(
            "{}{}",
            base_url.trim_end_matches('/'),
            CHAT_COMPLETIONS_PATH
        );
        // Fail fast at startup on an unusable endpoint (constitution: invalid
        // critical configuration fails fast) rather than per request.
        reqwest::Url::parse(&endpoint).map_err(|error| OpenRouterProviderError::InvalidEndpoint {
            detail: error.to_string(),
        })?;
        Ok(Self {
            client,
            endpoint,
            api_key: api_key.map(ApiKey::new),
        })
    }

    /// The endpoint requests are sent to. Exposed for operator-facing tests.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Whether a credential is present. Exposed so tests can assert the
    /// no-credential refusal happens without a network call.
    pub fn has_credential(&self) -> bool {
        self.api_key.is_some()
    }

    /// Sends the validated request and maps the provider's answer.
    async fn chat_impl(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
        let Some(api_key) = &self.api_key else {
            // No credential: refuse locally, without a network call, so a
            // misconfigured gateway never bills or leaks anything upstream.
            return Err(ProviderFailure::Refused);
        };

        let payload = map_request(request);
        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", api_key.as_str()))
            .json(&payload)
            .send()
            .await
            .map_err(classify_transport)?;

        if !response.status().is_success() {
            // Drain so the connection can be pooled; the body is refused.
            let _ = response.text().await;
            return Err(ProviderFailure::Refused);
        }

        let body = response
            .text()
            .await
            .map_err(|_| ProviderFailure::Unreachable)?;
        map_response(&body)
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
        self.chat_impl(request).await
    }

    fn id(&self) -> &str {
        OPENROUTER_ID
    }
}

/// Why the adapter could not be built.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpenRouterProviderError {
    #[error("failed to construct the OpenRouter HTTP client: {detail}")]
    ClientConstruction { detail: String },
    #[error("the OpenRouter base URL does not form a valid endpoint: {detail}")]
    InvalidEndpoint { detail: String },
}

// ---------------------------------------------------------------------------
// Request mapping: gateway ChatRequest -> OpenRouter chat-completion body
//
// The mapping lives in this adapter only (Phase 4 provider contract FR-022).
// `stream` is deliberately absent: the validation stage refuses streaming
// requests before any provider is contacted, so the adapter never sends one.
// Omitted controls are omitted from the body; no default is invented.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

fn map_request(request: &ChatRequest) -> OpenRouterChatRequest {
    OpenRouterChatRequest {
        model: request.model.clone(),
        messages: request
            .messages
            .iter()
            .map(|message| OpenRouterMessage {
                role: message.role.to_string(),
                content: message.content.clone(),
            })
            .collect(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
    }
}

// ---------------------------------------------------------------------------
// Response mapping: OpenRouter chat-completion body -> gateway ChatResponse
//
// A 200 with an `error` object (OpenRouter's way of reporting an upstream
// provider fault inside a success envelope) is a refusal, not a completion.
// A success without a usable first completion is `InvalidResponse`; a body
// that is not a response at all is `UnusableResponse` (Phase 4 provider
// contract, "Choosing a category").
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OpenRouterChatResponse {
    #[serde(default)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    choices: Option<Vec<OpenRouterChoice>>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    #[serde(default)]
    message: Option<OpenRouterMessageBlock>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterMessageBlock {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

fn map_response(body: &str) -> Result<ChatResponse, ProviderFailure> {
    let parsed: OpenRouterChatResponse = serde_json::from_str(body)
        .map_err(|_| ProviderFailure::UnusableResponse)?;

    if parsed.error.is_some() {
        return Err(ProviderFailure::Refused);
    }

    let content = parsed
        .choices
        .and_then(|choices| choices.into_iter().next())
        .and_then(|choice| choice.message)
        .and_then(|message| message.content)
        .filter(|content| !content.is_empty())
        .ok_or(ProviderFailure::InvalidResponse)?;

    let usage = parsed.usage.and_then(|usage| {
        match (usage.prompt_tokens, usage.completion_tokens) {
            (Some(prompt), Some(completion)) => Some(Usage::new(prompt, completion)),
            _ => None,
        }
    });

    Ok(ChatResponse { content, usage })
}

/// A transport failure is a deadline when the client's own bound fired and an
/// unreachable provider otherwise (DNS, connection refused, TLS, dropped body).
fn classify_transport(error: reqwest::Error) -> ProviderFailure {
    if error.is_timeout() {
        ProviderFailure::DeadlineExceeded
    } else {
        ProviderFailure::Unreachable
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::{
        Json, Router,
        body::Body,
        extract::State,
        http::{HeaderMap, Request, StatusCode},
        routing::post,
    };
    use serde_json::{Value, json};
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    use super::*;
    use crate::domain::{Message, MessageRole};

    fn request(model: &str, content: &str) -> ChatRequest {
        ChatRequest::new(model, vec![Message::new(MessageRole::User, content)])
    }

    fn request_with_controls(model: &str, content: &str) -> ChatRequest {
        ChatRequest::with_controls(
            model,
            vec![
                Message::new(MessageRole::System, "Be terse."),
                Message::new(MessageRole::User, content),
            ],
            Some(0.7),
            Some(500),
        )
    }

    // -----------------------------------------------------------------------
    // Request mapping
    // -----------------------------------------------------------------------

    #[test]
    fn map_request_serializes_the_documented_openrouter_shape() {
        let body = serde_json::to_value(map_request(&request_with_controls("openai/gpt-4o", "Hello")))
            .unwrap();

        assert_eq!(
            body,
            json!({
                "model": "openai/gpt-4o",
                "messages": [
                    {"role": "system", "content": "Be terse."},
                    {"role": "user", "content": "Hello"},
                ],
                "temperature": 0.7,
                "max_tokens": 500,
            })
        );
    }

    #[test]
    fn map_request_omits_unspecified_controls_and_the_stream_flag() {
        let body = serde_json::to_value(map_request(&request("mock-model", "Hello")))
            .unwrap();
        let object = body.as_object().unwrap();

        assert_eq!(object.len(), 2);
        assert!(!object.contains_key("temperature"));
        assert!(!object.contains_key("max_tokens"));
        assert!(!object.contains_key("stream"));
    }

    #[test]
    fn map_request_keeps_message_order_and_role_spelling() {
        let chat = ChatRequest::with_controls(
            "mock-model",
            vec![
                Message::new(MessageRole::Assistant, "first"),
                Message::new(MessageRole::System, "second"),
                Message::new(MessageRole::User, "third"),
            ],
            None,
            None,
        );
        let body = serde_json::to_value(map_request(&chat)).unwrap();

        assert_eq!(
            body["messages"],
            json!([
                {"role": "assistant", "content": "first"},
                {"role": "system", "content": "second"},
                {"role": "user", "content": "third"},
            ])
        );
    }

    #[test]
    fn map_request_preserves_untrimmed_model_and_content() {
        let chat = ChatRequest::new(
            "  spaced / model  ",
            vec![Message::new(MessageRole::User, "  content  ")],
        );
        let body = serde_json::to_value(map_request(&chat)).unwrap();

        assert_eq!(body["model"], json!("  spaced / model  "));
        assert_eq!(body["messages"][0]["content"], json!("  content  "));
    }

    // -----------------------------------------------------------------------
    // Response mapping
    // -----------------------------------------------------------------------

    #[test]
    fn map_response_maps_content_and_usage() {
        let body = r#"{
            "id": "gen-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "openai/gpt-4o",
            "choices": [
                {"index": 0, "message": {"role": "assistant", "content": "Hi there!"}, "finish_reason": "stop"}
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#;

        let response = map_response(body).unwrap();

        assert_eq!(response.content, "Hi there!");
        assert_eq!(
            response.usage,
            Some(Usage::new(10, 5)),
            "usage total is derived, not copied"
        );
    }

    #[test]
    fn map_response_reports_no_usage_when_the_provider_sent_none() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "Hi"}}]}"#;

        let response = map_response(body).unwrap();

        assert_eq!(response.content, "Hi");
        assert!(response.usage.is_none(), "no usage is invented");
    }

    #[test]
    fn map_response_derives_the_total_from_prompt_plus_completion() {
        // OpenRouter sometimes omits total_tokens; the domain invariant
        // (total = prompt + completion) is authoritative.
        let body = r#"{"choices": [{"message": {"content": "Hi"}}], "usage": {"prompt_tokens": 7, "completion_tokens": 3}}"#;

        let usage = map_response(body).unwrap().usage.unwrap();

        assert_eq!(usage.prompt_tokens, 7);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 10);
    }

    #[test]
    fn map_response_refuses_a_partial_usage_block() {
        for usage in [r#"{"prompt_tokens": 7}"#, r#"{"completion_tokens": 3}"#] {
            let body = format!(
                r#"{{"choices": [{{"message": {{"content": "Hi"}}}}], "usage": {usage}}}"#
            );

            assert!(
                map_response(&body).unwrap().usage.is_none(),
                "partial usage {usage} must not become invented counts"
            );
        }
    }

    #[test]
    fn map_response_treats_a_success_envelope_with_an_error_object_as_a_refusal() {
        let body = r#"{"error": {"message": "upstream provider failed", "code": 502}}"#;

        assert_eq!(map_response(body).unwrap_err(), ProviderFailure::Refused);
    }

    #[test]
    fn map_response_reports_invalid_when_no_completion_is_present() {
        for body in [
            r#"{"id": "x", "choices": []}"#,
            r#"{"id": "x", "choices": [{"message": {}}]}"#,
            r#"{"id": "x", "choices": [{"message": {"content": ""}}]}"#,
            r#"{"id": "x", "choices": [{"message": {"content": null}}]}"#,
            r#"{"id": "x"}"#,
        ] {
            assert_eq!(
                map_response(body).unwrap_err(),
                ProviderFailure::InvalidResponse,
                "body {body}"
            );
        }
    }

    #[test]
    fn map_response_reports_unusable_when_the_body_is_not_a_response() {
        for body in [
            "not json at all",
            r#"["an", "array"]"#,
            r#""just a string""#,
            "5",
        ] {
            assert_eq!(
                map_response(body).unwrap_err(),
                ProviderFailure::UnusableResponse,
                "body {body}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Credential handling
    // -----------------------------------------------------------------------

    #[test]
    fn the_endpoint_is_the_base_url_with_the_chat_path() {
        let provider = OpenRouterProvider::new("https://openrouter.ai/api/v1", Some("k".to_owned()))
            .unwrap();
        assert_eq!(provider.endpoint(), "https://openrouter.ai/api/v1/chat/completions");

        let trailing = OpenRouterProvider::new("https://openrouter.ai/api/v1/", Some("k".to_owned()))
            .unwrap();
        assert_eq!(
            trailing.endpoint(),
            "https://openrouter.ai/api/v1/chat/completions",
            "a trailing slash must not produce a double slash"
        );
    }

    #[test]
    fn the_credential_is_redacted_from_debug_output() {
        let sentinel = "SENTINEL_KEY_VALUE_XYZ";
        let provider = OpenRouterProvider::new("https://openrouter.ai/api/v1", Some(sentinel.to_owned()))
            .unwrap();

        let rendered = format!("{provider:?}");

        assert!(!rendered.contains(sentinel), "leaked the credential in {rendered}");
        assert!(rendered.contains("<redacted>"));
    }

    // -----------------------------------------------------------------------
    // Live behaviour against a local stand-in for the OpenRouter API
    // -----------------------------------------------------------------------

    /// A stand-in OpenRouter: records every request and replies per its mode.
    #[derive(Clone)]
    struct FakeOpenRouter {
        mode: Mode,
        requests: std::sync::Arc<AtomicUsize>,
        last_authorization: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        last_body: std::sync::Arc<std::sync::Mutex<Option<Value>>>,
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Success,
        Status(StatusCode),
        UnusableBody,
        MissingCompletion,
    }

    async fn fake_handler(
        State(fake): State<FakeOpenRouter>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Result<Json<Value>, StatusCode> {
        fake.requests.fetch_add(1, Ordering::SeqCst);
        *fake.last_authorization.lock().unwrap() =
            headers.get("Authorization").and_then(|value| value.to_str().ok()).map(str::to_owned);
        *fake.last_body.lock().unwrap() = Some(body);

        match fake.mode {
            Mode::Success => {
                let body_value = json!({
                    "id": "gen-1",
                    "object": "chat.completion",
                    "created": 1700000000,
                    "model": "echo/model",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "fake completion"},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}
                });
                Ok(Json(body_value))
            }
            Mode::Status(status) => Err(status),
            Mode::UnusableBody => {
                Ok(Json(Value::String("this is not json".to_owned())))
            }
            Mode::MissingCompletion => {
                let body_value = json!({"id": "gen-2", "choices": []});
                Ok(Json(body_value))
            }
        }
    }

    async fn start_fake(mode: Mode) -> (OpenRouterProvider, String, FakeOpenRouter) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fake = FakeOpenRouter {
            mode,
            requests: std::sync::Arc::new(AtomicUsize::new(0)),
            last_authorization: std::sync::Arc::new(std::sync::Mutex::new(None)),
            last_body: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let app = Router::new()
            .route(CHAT_COMPLETIONS_PATH, post(fake_handler))
            .with_state(fake.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let base_url = format!("http://{address}");
        let provider = OpenRouterProvider::new(&base_url, Some("test-key-123".to_owned())).unwrap();
        (provider, base_url, fake)
    }

    #[tokio::test]
    async fn a_validated_request_round_trips_through_the_provider() {
        let (provider, _, fake) = start_fake(Mode::Success).await;

        let response = provider
            .chat(&request_with_controls("echo/model", "Hello"))
            .await
            .unwrap();

        assert_eq!(response.content, "fake completion");
        assert_eq!(response.usage, Some(Usage::new(4, 2)));

        let sent = fake.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["model"], json!("echo/model"));
        assert_eq!(sent["messages"][1]["role"], json!("user"));
        assert_eq!(sent["messages"][1]["content"], json!("Hello"));
        assert_eq!(sent["temperature"], json!(0.7));
        assert_eq!(sent["max_tokens"], json!(500));
        assert!(sent.get("stream").is_none());
    }

    #[tokio::test]
    async fn the_credential_is_sent_as_a_bearer_token_and_nothing_else() {
        let (provider, _, fake) = start_fake(Mode::Success).await;

        let _ = provider.chat(&request("m", "hi")).await.unwrap();

        let authorization = fake.last_authorization.lock().unwrap().clone().unwrap();
        assert_eq!(authorization, "Bearer test-key-123");

        // The key appears exactly once, in the Authorization header; the body
        // the provider received is the mapped request and contains no key.
        let body = fake.last_body.lock().unwrap().clone().unwrap().to_string();
        assert!(!body.contains("test-key-123"));
    }

    #[tokio::test]
    async fn non_success_status_is_a_refusal() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
        ] {
            let (provider, _, _) = start_fake(Mode::Status(status)).await;

            let result = provider.chat(&request("m", "hi")).await;

            assert_eq!(
                result.unwrap_err(),
                ProviderFailure::Refused,
                "status {status:?} must be a refusal"
            );
        }
    }

    #[tokio::test]
    async fn a_non_json_success_body_is_unusable() {
        let (provider, _, _) = start_fake(Mode::UnusableBody).await;

        assert_eq!(
            provider.chat(&request("m", "hi")).await.unwrap_err(),
            ProviderFailure::UnusableResponse
        );
    }

    #[tokio::test]
    async fn a_success_without_a_completion_is_invalid() {
        let (provider, _, _) = start_fake(Mode::MissingCompletion).await;

        assert_eq!(
            provider.chat(&request("m", "hi")).await.unwrap_err(),
            ProviderFailure::InvalidResponse
        );
    }

    #[tokio::test]
    async fn a_connection_refusal_is_unreachable() {
        // Nothing listens on this port.
        let provider = OpenRouterProvider::new("http://127.0.0.1:1", Some("k".to_owned()))
            .unwrap();

        assert_eq!(
            provider.chat(&request("m", "hi")).await.unwrap_err(),
            ProviderFailure::Unreachable
        );
    }

    #[tokio::test]
    async fn a_missing_credential_refuses_without_any_network_call() {
        let fake = FakeOpenRouter {
            mode: Mode::Success,
            requests: std::sync::Arc::new(AtomicUsize::new(0)),
            last_authorization: std::sync::Arc::new(std::sync::Mutex::new(None)),
            last_body: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route(CHAT_COMPLETIONS_PATH, post(fake_handler))
            .with_state(fake.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let provider = OpenRouterProvider::new(&format!("http://{address}"), None).unwrap();
        assert!(!provider.has_credential());

        assert_eq!(
            provider.chat(&request("m", "hi")).await.unwrap_err(),
            ProviderFailure::Refused
        );
        assert_eq!(fake.requests.load(Ordering::SeqCst), 0, "no upstream call may be made");
    }

    #[tokio::test]
    async fn from_config_reads_the_key_from_the_named_variable_when_absent() {
        // A variable name no test sets; the point is that the adapter does not
        // fall over and stays selectable-but-refusing without a credential.
        let config = OpenRouterConfig::from_values(
            None,
            Some("http://127.0.0.1:1"),
            Some("AI_GATEWAY_TEST_KEY_THAT_IS_NEVER_SET_9371"),
        )
        .unwrap();

        let provider = OpenRouterProvider::from_config(&config).unwrap();

        assert!(!provider.has_credential());
        assert_eq!(
            provider.chat(&request("m", "hi")).await.unwrap_err(),
            ProviderFailure::Refused
        );
    }

    #[tokio::test]
    async fn an_invalid_base_url_fails_fast_at_construction() {
        assert!(OpenRouterProvider::new("not a url", Some("k".to_owned())).is_err());
    }
}
