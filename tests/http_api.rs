use std::sync::Arc;

use ai_gateway::{api::error::ApiError, api::server::build_router, application::AppState};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{
        Method, Request, StatusCode,
        header::{ALLOW, CONTENT_TYPE},
    },
    response::Response,
    routing::post,
};
use serde_json::{Value, json};
use tokio::{sync::Barrier, task::JoinSet};
use tower::ServiceExt;

fn health_request() -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap()
}

async fn assert_health_contract(response: Response) {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), br#"{"status":"ok"}"#);
}

fn readiness_request() -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri("/ready")
        .body(Body::empty())
        .unwrap()
}

async fn assert_readiness_contract(response: Response, status: StatusCode, expected: &[u8]) {
    assert_eq!(response.status(), status);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), expected);
}

#[tokio::test]
async fn health_returns_exact_contract_without_provider_configuration() {
    let router = build_router(AppState::new("test"));

    let response = router.oneshot(health_request()).await.unwrap();

    assert_health_contract(response).await;
}

#[tokio::test]
async fn health_remains_available_across_lifecycle_phases() {
    let state = AppState::new("test");
    let router = build_router(state.clone());

    let initializing = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(initializing).await;
    state.lifecycle.mark_ready();
    let ready = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(ready).await;
    state.lifecycle.begin_shutdown();
    let shutting_down = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(shutting_down).await;
    state.lifecycle.mark_stopped();
    let stopped = router.oneshot(health_request()).await.unwrap();
    assert_health_contract(stopped).await;
}

#[tokio::test]
async fn health_supports_repeated_requests() {
    let router = build_router(AppState::new("test"));

    for _ in 0..20 {
        let response = router.clone().oneshot(health_request()).await.unwrap();
        assert_health_contract(response).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_health_requests_remain_isolated() {
    let router: Router = build_router(AppState::new("test"));
    let mut requests = JoinSet::new();

    for _ in 0..32 {
        let router = router.clone();
        requests.spawn(async move {
            let response = router.oneshot(health_request()).await.unwrap();
            assert_health_contract(response).await;
        });
    }

    while let Some(result) = requests.join_next().await {
        result.unwrap();
    }
}

#[tokio::test]
async fn initializing_reports_liveness_and_not_ready_for_twenty_requests() {
    let router = build_router(AppState::new("test"));

    for _ in 0..20 {
        let health = router.clone().oneshot(health_request()).await.unwrap();
        assert_health_contract(health).await;
        let readiness = router.clone().oneshot(readiness_request()).await.unwrap();
        assert_readiness_contract(
            readiness,
            StatusCode::SERVICE_UNAVAILABLE,
            br#"{"status":"not_ready"}"#,
        )
        .await;
    }
}

#[tokio::test]
async fn ready_reports_liveness_and_ready_for_twenty_requests() {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);

    for _ in 0..20 {
        let health = router.clone().oneshot(health_request()).await.unwrap();
        assert_health_contract(health).await;
        let readiness = router.clone().oneshot(readiness_request()).await.unwrap();
        assert_readiness_contract(readiness, StatusCode::OK, br#"{"status":"ready"}"#).await;
    }
}

#[tokio::test]
async fn shutting_down_reports_liveness_and_not_ready_for_twenty_requests() {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    state.lifecycle.begin_shutdown();
    let router = build_router(state);

    for _ in 0..20 {
        let health = router.clone().oneshot(health_request()).await.unwrap();
        assert_health_contract(health).await;
        let readiness = router.clone().oneshot(readiness_request()).await.unwrap();
        assert_readiness_contract(
            readiness,
            StatusCode::SERVICE_UNAVAILABLE,
            br#"{"status":"not_ready"}"#,
        )
        .await;
    }
}

fn ready_router() -> Router {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    build_router(state)
}

fn chat_request(body: &Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

async fn parse_json(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&body).unwrap();
    (status, value)
}

fn canonical_chat_body() -> Value {
    json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello"}]
    })
}

fn minimum_invalid_bodies() -> Vec<Value> {
    vec![
        json!({
            "model": "",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
        json!({
            "model": "mock-model",
            "messages": []
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "tool", "content": "Hello"}]
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": ""}]
        }),
    ]
}

fn expected_success(model: &str) -> Value {
    json!({
        "id": "chat_mock",
        "object": "chat.completion",
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "This is a mocked chat completion."
                },
                "finish_reason": "stop"
            }
        ]
    })
}

fn assert_error_envelope(value: &Value, code: &str, message: &str) {
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 2);
    assert_eq!(object.get("code").and_then(Value::as_str), Some(code));
    assert_eq!(object.get("message").and_then(Value::as_str), Some(message));
    assert!(!object.contains_key("details"));
    assert!(!object.contains_key("error"));
}

#[tokio::test]
async fn canonical_chat_completion_returns_exact_envelope() {
    let router = ready_router();

    let response = router
        .oneshot(chat_request(&canonical_chat_body()))
        .await
        .unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));

    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 4);
    assert!(!object.contains_key("usage"));
    assert!(!object.contains_key("created"));

    let choices = value["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 1);
    let choice = choices[0].as_object().unwrap();
    assert_eq!(choice.len(), 3);
    assert_eq!(choice["message"].as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn chat_accepts_all_supported_roles() {
    let router = ready_router();

    for role in ["system", "user", "assistant"] {
        let body = json!({
            "model": "mock-model",
            "messages": [{"role": role, "content": "Hello"}]
        });

        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "role {role} was rejected");
        assert_eq!(value, expected_success("mock-model"));
    }
}

#[tokio::test]
async fn chat_accepts_unknown_model_and_echoes_it() {
    let router = ready_router();
    let model = "vendor/UNREGISTERED-\u{03a9}-7f3c";
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let response = router.oneshot(chat_request(&body)).await.unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success(model));
}

#[tokio::test]
async fn chat_ignores_future_and_optional_fields() {
    let router = ready_router();
    let body = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello", "name": "ignored"}],
        "stream": false,
        "temperature": -10.0,
        "max_tokens": -1,
        "n": 9,
        "top_p": 2.0,
        "future": {"anything": true}
    });

    let response = router.oneshot(chat_request(&body)).await.unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));
    assert_eq!(value["choices"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn chat_rejects_minimum_invalid_fields() {
    let router = ready_router();

    for body in minimum_invalid_bodies() {
        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "body accepted: {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn chat_validates_minimum_fields_before_streaming() {
    let router = ready_router();

    for mut body in minimum_invalid_bodies() {
        body["stream"] = Value::Bool(true);

        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "body accepted: {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn chat_rejects_streaming_when_minimum_valid() {
    let router = ready_router();
    let mut body = canonical_chat_body();
    body["stream"] = Value::Bool(true);

    let response = router.oneshot(chat_request(&body)).await.unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error_envelope(&value, "unsupported_feature", "Streaming is not supported.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_chat_is_deterministic_for_one_hundred_requests() {
    let router = ready_router();
    let body = canonical_chat_body();
    let mut envelopes: Vec<Value> = Vec::new();

    for _ in 0..100 {
        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value, expected_success("mock-model"));
        envelopes.push(value);
    }

    let first = &envelopes[0];
    for value in &envelopes {
        assert_eq!(value, first);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_chat_requests_remain_isolated() {
    let router = ready_router();
    let barrier = Arc::new(Barrier::new(2));
    let mut requests = JoinSet::new();

    let cases = [
        (
            "concurrent/model-alpha-8c1",
            "ALPHA_ONLY_7F3C",
            "BETA_ONLY_5A2E",
            "concurrent/model-beta-4d9",
        ),
        (
            "concurrent/model-beta-4d9",
            "BETA_ONLY_5A2E",
            "ALPHA_ONLY_7F3C",
            "concurrent/model-alpha-8c1",
        ),
    ];

    for (model, sentinel, foreign_sentinel, foreign_model) in cases {
        let router = router.clone();
        let barrier = barrier.clone();
        requests.spawn(async move {
            let body = json!({
                "model": model,
                "messages": [{"role": "user", "content": sentinel}]
            });

            barrier.wait().await;
            let response = router.oneshot(chat_request(&body)).await.unwrap();

            let (status, value) = parse_json(response).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(value, expected_success(model));

            let serialized = value.to_string();
            assert!(!serialized.contains(foreign_sentinel));
            assert!(!serialized.contains(foreign_model));
        });
    }

    while let Some(result) = requests.join_next().await {
        result.unwrap();
    }
}

const CANONICAL_CHAT_BODY: &str =
    r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}"#;

const FORBIDDEN_ERROR_BODY_FRAGMENTS: [&str; 12] = [
    "SENTINEL_PROMPT",
    "SENTINEL_MODEL",
    "api_key",
    "password",
    "Failed to",
    "expected",
    "serde",
    "line ",
    "column",
    "source",
    "diagnostic",
    "backtrace",
];

fn error_request(
    method: Method,
    uri: &str,
    content_type: Option<&str>,
    body: &str,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);

    if let Some(content_type) = content_type {
        builder = builder.header(CONTENT_TYPE, content_type);
    }

    builder.body(Body::from(body.to_owned())).unwrap()
}

fn allow_header(response: &Response) -> Option<String> {
    response
        .headers()
        .get(ALLOW)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn content_type_header(response: &Response) -> Option<String> {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

async fn assert_error_contract(response: Response, status: StatusCode, code: &str, message: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(
        content_type_header(&response).as_deref(),
        Some("application/json")
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let raw = String::from_utf8_lossy(&body).into_owned();
    let value: Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("error body is not valid JSON: {raw} ({error})"));

    assert_eq!(value, json!({"code": code, "message": message}));
    assert_eq!(value.as_object().map(serde_json::Map::len), Some(2));

    for fragment in FORBIDDEN_ERROR_BODY_FRAGMENTS {
        assert!(
            !raw.contains(fragment),
            "error body leaked {fragment}: {raw}"
        );
    }
}

async fn assert_head_error_contract(response: Response, status: StatusCode, allow: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(allow_header(&response).as_deref(), Some(allow));
    assert_eq!(
        content_type_header(&response).as_deref(),
        Some("application/json")
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(body.is_empty(), "HEAD response carried a body: {body:?}");
}

#[derive(Clone, Copy)]
enum NotReadyPhase {
    Initializing,
    ShuttingDown,
    Stopped,
}

fn state_in_phase(phase: NotReadyPhase) -> AppState {
    let state = AppState::new("test");

    if !matches!(phase, NotReadyPhase::Initializing) {
        state.lifecycle.mark_ready();
        state.lifecycle.begin_shutdown();
    }

    if matches!(phase, NotReadyPhase::Stopped) {
        state.lifecycle.mark_stopped();
    }

    state
}

fn router_in_phase(phase: NotReadyPhase) -> Router {
    build_router(state_in_phase(phase))
}

async fn failing_handler() -> Result<Json<Value>, ApiError> {
    Err(ApiError::InternalError)
}

fn test_internal_failure_router() -> Router {
    Router::new().route("/v1/chat/completions", post(failing_handler))
}

#[tokio::test]
async fn chat_rejects_malformed_empty_and_non_object_bodies() {
    let router = ready_router();
    let bodies = [
        "",
        "   ",
        "{",
        r#"{"model":"#,
        "not json",
        "null",
        "[]",
        r#"[{"model":"m"}]"#,
        r#""a string""#,
        "123",
        "true",
    ];

    for body in bodies {
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            Some("application/json"),
            body,
        );

        let response = router.clone().oneshot(request).await.unwrap();

        assert_error_contract(
            response,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;
    }
}

#[tokio::test]
async fn chat_rejects_missing_null_and_wrong_typed_fields() {
    let router = ready_router();
    let bodies = [
        "{}",
        r#"{"model":"SENTINEL_MODEL"}"#,
        r#"{"messages":[{"role":"user","content":"SENTINEL_PROMPT"}]}"#,
        r#"{"model":null,"messages":[{"role":"user","content":"x"}]}"#,
        r#"{"model":123,"messages":[{"role":"user","content":"x"}]}"#,
        r#"{"model":"m","messages":null}"#,
        r#"{"model":"m","messages":{}}"#,
        r#"{"model":"m","messages":"x"}"#,
        r#"{"model":"m","messages":[null]}"#,
        r#"{"model":"m","messages":["x"]}"#,
        r#"{"model":"m","messages":[{"content":"SENTINEL_PROMPT"}]}"#,
        r#"{"model":"m","messages":[{"role":"user"}]}"#,
        r#"{"model":"m","messages":[{"role":1,"content":"x"}]}"#,
        r#"{"model":"m","messages":[{"role":"user","content":null}]}"#,
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"stream":"yes"}"#,
        r#"{"model":"m","messages":[{"role":"user","content":"x"}],"stream":null}"#,
    ];

    for body in bodies {
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            Some("application/json"),
            body,
        );

        let response = router.clone().oneshot(request).await.unwrap();

        assert_error_contract(
            response,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;
    }
}

#[tokio::test]
async fn structurally_invalid_input_precedes_streaming_rejection() {
    let router = ready_router();
    let bodies = [
        r#"{"stream":true}"#,
        r#"{"model":1,"stream":true}"#,
        r#"{"model":"m","messages":[],"stream":true}"#,
    ];

    for body in bodies {
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            Some("application/json"),
            body,
        );

        let response = router.clone().oneshot(request).await.unwrap();

        assert_error_contract(
            response,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "The chat request is invalid.",
        )
        .await;
    }
}

#[tokio::test]
async fn chat_rejects_missing_and_unsupported_media_types() {
    let router = ready_router();
    let content_types = [
        None,
        Some("text/plain"),
        Some("text/json"),
        Some("application/x-www-form-urlencoded"),
        Some("application/xml"),
    ];

    for content_type in content_types {
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            content_type,
            CANONICAL_CHAT_BODY,
        );

        let response = router.clone().oneshot(request).await.unwrap();

        assert_error_contract(
            response,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "The request media type is not supported.",
        )
        .await;
    }
}

#[tokio::test]
async fn valid_request_with_json_charset_parameter_succeeds() {
    let router = ready_router();
    let request = error_request(
        Method::POST,
        "/v1/chat/completions",
        Some("application/json; charset=utf-8"),
        CANONICAL_CHAT_BODY,
    );

    let response = router.oneshot(request).await.unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));
}

#[tokio::test]
async fn admission_precedes_media_type_and_body_handling() {
    let phases = [
        NotReadyPhase::Initializing,
        NotReadyPhase::ShuttingDown,
        NotReadyPhase::Stopped,
    ];

    for phase in phases {
        let router = router_in_phase(phase);
        let cases = [
            (None, CANONICAL_CHAT_BODY),
            (Some("text/plain"), CANONICAL_CHAT_BODY),
            (Some("application/json"), "{"),
            (Some("application/json"), CANONICAL_CHAT_BODY),
        ];

        for (content_type, body) in cases {
            let request = error_request(Method::POST, "/v1/chat/completions", content_type, body);

            let response = router.clone().oneshot(request).await.unwrap();

            assert_error_contract(
                response,
                StatusCode::SERVICE_UNAVAILABLE,
                "not_ready",
                "The gateway is not accepting new chat requests.",
            )
            .await;
        }
    }
}

#[tokio::test]
async fn routing_precedes_admission() {
    let router = router_in_phase(NotReadyPhase::ShuttingDown);
    let cases = [
        (Method::GET, "/v1/chat/completions", "POST"),
        (Method::POST, "/health", "GET"),
        (Method::POST, "/ready", "GET"),
    ];

    for (method, path, allow) in cases {
        let request = error_request(method, path, Some("application/json"), "");

        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(allow_header(&response).as_deref(), Some(allow));

        assert_error_contract(
            response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        )
        .await;
    }
}

#[tokio::test]
async fn unsupported_methods_on_known_paths_return_405_with_allow() {
    let router = ready_router();
    let get_only_methods = [
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
    ];

    for path in ["/health", "/ready"] {
        for method in &get_only_methods {
            let request = error_request(method.clone(), path, Some("application/json"), "");

            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(allow_header(&response).as_deref(), Some("GET"));

            assert_error_contract(
                response,
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "The request method is not allowed for this path.",
            )
            .await;
        }
    }

    let post_only_methods = [
        Method::GET,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
    ];

    for method in post_only_methods {
        let request = error_request(method, "/v1/chat/completions", Some("application/json"), "");

        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(allow_header(&response).as_deref(), Some("POST"));

        assert_error_contract(
            response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        )
        .await;
    }
}

#[tokio::test]
async fn head_requests_suppress_response_bodies() {
    let router = ready_router();
    let cases = [
        ("/health", "GET"),
        ("/ready", "GET"),
        ("/v1/chat/completions", "POST"),
    ];

    for (path, allow) in cases {
        let request = error_request(Method::HEAD, path, Some("application/json"), "");

        let response = router.clone().oneshot(request).await.unwrap();

        assert_head_error_contract(response, StatusCode::METHOD_NOT_ALLOWED, allow).await;
    }
}

#[tokio::test]
async fn unknown_paths_return_404_contract() {
    let router = ready_router();
    let paths = [
        "/",
        "/healthz",
        "/Health",
        "/v1",
        "/v1/chat",
        "/v1/chat/completions/extra",
        "/ready/",
        "//health",
        "/v2/chat/completions",
    ];

    for path in paths {
        for method in [Method::GET, Method::POST, Method::HEAD, Method::DELETE] {
            let request = error_request(method.clone(), path, Some("application/json"), "");

            let response = router.clone().oneshot(request).await.unwrap();

            if method == Method::HEAD {
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "path {path}");
                assert_eq!(
                    content_type_header(&response).as_deref(),
                    Some("application/json"),
                    "path {path}"
                );

                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                assert!(body.is_empty(), "path {path} carried a HEAD body: {body:?}");
            } else {
                assert_error_contract(
                    response,
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "The requested path was not found.",
                )
                .await;
            }
        }
    }
}

#[tokio::test]
async fn chat_is_rejected_while_not_ready() {
    let phases = [
        NotReadyPhase::Initializing,
        NotReadyPhase::ShuttingDown,
        NotReadyPhase::Stopped,
    ];

    for phase in phases {
        let router = router_in_phase(phase);
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            Some("application/json"),
            CANONICAL_CHAT_BODY,
        );

        let response = router.oneshot(request).await.unwrap();

        assert_error_contract(
            response,
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "The gateway is not accepting new chat requests.",
        )
        .await;
    }
}

#[tokio::test]
async fn injected_internal_failure_returns_500_contract() {
    let router = test_internal_failure_router();
    let request = error_request(
        Method::POST,
        "/v1/chat/completions",
        Some("application/json"),
        CANONICAL_CHAT_BODY,
    );

    let response = router.oneshot(request).await.unwrap();

    assert_error_contract(
        response,
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "The gateway could not complete the request.",
    )
    .await;
}
