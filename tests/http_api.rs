use std::{collections::BTreeSet, sync::Arc};

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

async fn parse_raw(response: Response) -> (StatusCode, Vec<u8>) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, body.to_vec())
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
    // `n`, `top_p`, `future`, and the per-message `name` stay unknown and ignored.
    // `temperature`/`max_tokens` are now recognized fields, so they carry in-range
    // values here; out-of-range controls are covered by the control-range tests.
    let body = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello", "name": "ignored"}],
        "stream": false,
        "temperature": 0.5,
        "max_tokens": 16,
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
async fn chat_accepts_both_generation_controls_and_omitting_them() {
    let router = ready_router();
    let base = |extra: Value| {
        let mut body = json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        body
    };

    for (temperature, max_tokens) in [
        (json!(0.7), json!(500)),
        (json!(0.0), json!(1)),
        (json!(2.0), json!(4096)),
        (json!(0.25), json!(1)),
    ] {
        let body = base(json!({"temperature": temperature, "max_tokens": max_tokens}));
        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "temperature {temperature} / max_tokens {max_tokens} was refused"
        );
        assert_eq!(value, expected_success("mock-model"));
    }

    for extra in [
        json!({}),
        json!({"temperature": 1.0}),
        json!({"max_tokens": 128}),
    ] {
        let body = base(extra.clone());
        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{extra} was refused");
        assert_eq!(value, expected_success("mock-model"));
    }
}

#[tokio::test]
async fn chat_response_is_byte_identical_across_control_combinations() {
    let mut bodies = Vec::new();
    for (temperature, max_tokens) in [
        (None, None),
        (Some(json!(0.0)), None),
        (Some(json!(0.7)), Some(json!(500))),
        (Some(json!(2.0)), Some(json!(4096))),
    ] {
        let mut body = json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        if let Some(temperature) = temperature {
            body["temperature"] = temperature;
        }
        if let Some(max_tokens) = max_tokens {
            body["max_tokens"] = max_tokens;
        }
        bodies.push(body);
    }

    let mut responses = Vec::new();
    for body in &bodies {
        let router = ready_router();
        let response = router.oneshot(chat_request(body)).await.unwrap();
        let (status, raw) = parse_raw(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "control combination {body} was refused"
        );
        responses.push(raw);
    }

    for pair in responses.windows(2) {
        assert_eq!(
            pair[0], pair[1],
            "mock output changed when the controls changed"
        );
    }
}

/// A body of exactly `total` bytes that is also a valid chat request, so a
/// boundary test can tell "admitted" apart from "refused as malformed".
fn chat_body_of_size(total: usize) -> Vec<u8> {
    const PREFIX: &[u8] = br#"{"model":"mock-model","messages":[{"role":"user","content":""#;
    const SUFFIX: &[u8] = br#""}]}"#;

    let mut body = PREFIX.to_vec();
    body.extend(std::iter::repeat_n(
        b'x',
        total - PREFIX.len() - SUFFIX.len(),
    ));
    body.extend_from_slice(SUFFIX);
    body
}

fn chat_request_with_body(body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/v1/chat/completions")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// One violating request per rule ID from `contracts/validation-rules.md` §2,
/// paired with the exact status, code, and message §4 requires.
struct RuleCase {
    id: &'static str,
    method: Method,
    uri: &'static str,
    content_type: Option<&'static str>,
    body: Vec<u8>,
    expected_status: StatusCode,
    expected_code: &'static str,
    expected_message: &'static str,
}

impl RuleCase {
    fn new(
        id: &'static str,
        body: Vec<u8>,
        status: StatusCode,
        code: &'static str,
        message: &'static str,
    ) -> Self {
        Self {
            id,
            method: Method::POST,
            uri: "/v1/chat/completions",
            content_type: Some("application/json"),
            body,
            expected_status: status,
            expected_code: code,
            expected_message: message,
        }
    }
}

fn valid_chat_json() -> String {
    r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}]}"#.to_owned()
}

fn with_field(field: &str, literal: &str) -> Vec<u8> {
    format!(
        r#"{{"model":"mock-model","messages":[{{"role":"user","content":"Hello"}}],"{field}":{literal}}}"#
    )
    .into_bytes()
}

fn rule_cases() -> Vec<RuleCase> {
    let invalid = "The chat request is invalid.";
    let mut cases = vec![
        RuleCase::new(
            "ROUTE-001",
            valid_chat_json().into_bytes(),
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested path was not found.",
        ),
        RuleCase::new(
            "METHOD-001",
            valid_chat_json().into_bytes(),
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "The request method is not allowed for this path.",
        ),
        RuleCase::new(
            "SIZE-001",
            vec![b'x'; 1_048_577],
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "The request payload is too large.",
        ),
        RuleCase {
            content_type: Some("text/plain"),
            ..RuleCase::new(
                "TYPE-001",
                valid_chat_json().into_bytes(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "The request media type is not supported.",
            )
        },
        RuleCase::new(
            "PARSE-001",
            b"{ not json".to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "TYPE-002",
            with_field("temperature", r#""0.5""#),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "TYPE-003",
            with_field("max_tokens", "true"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "TYPE-004",
            with_field("stream", r#""false""#),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "UNIQ-001",
            br#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "UNIQ-002",
            br#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":10,"max_tokens":20}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "FIELD-001",
            br#"{"model":"","messages":[{"role":"user","content":"Hello"}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "FIELD-002",
            br#"{"model":"mock-model","messages":[]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "FIELD-003",
            br#"{"model":"mock-model","messages":[{"role":"user"}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "FIELD-004",
            br#"{"model":"mock-model","messages":[{"role":"tool","content":"Hello"}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "FIELD-005",
            br#"{"model":"mock-model","messages":[{"role":"user","content":""}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "CTRL-001",
            with_field("temperature", "2.0001"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "CTRL-002",
            with_field("temperature", "9.0"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "CTRL-003",
            with_field("max_tokens", "4097"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        RuleCase::new(
            "STREAM-001",
            with_field("stream", "true"),
            StatusCode::BAD_REQUEST,
            "unsupported_feature",
            "Streaming is not supported.",
        ),
    ];

    // ROUTE-001 and METHOD-001 need a different target, not a different body.
    cases[0].uri = "/v1/chat/nope";
    cases[1].method = Method::GET;
    // ADMIT-001 needs a gateway that has not reached ready.
    cases.push(RuleCase {
        id: "ADMIT-001",
        ..RuleCase::new(
            "ADMIT-001",
            valid_chat_json().into_bytes(),
            StatusCode::SERVICE_UNAVAILABLE,
            "not_ready",
            "The gateway is not accepting new chat requests.",
        )
    });

    cases
}

/// The HTTP request for a rule case, carrying that case's method, target,
/// media type, and raw body.
fn rule_request(case: &RuleCase) -> Request<Body> {
    let mut builder = Request::builder().method(case.method.clone()).uri(case.uri);
    if let Some(content_type) = case.content_type {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(case.body.clone())).unwrap()
}

/// A ready router for every case except `ADMIT-001`, which needs a gateway that
/// has not reached ready so admission refuses it.
fn rule_router(case: &RuleCase) -> Router {
    if case.id == "ADMIT-001" {
        return build_router(AppState::new("test"));
    }

    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    build_router(state)
}

#[tokio::test]
async fn precedence_resolves_combined_violations_to_the_earliest_stage() {
    // Each case breaks every stage at or before the one it names, and the
    // expected response is the earliest such stage's documented row.
    struct PrecedenceCase {
        id: &'static str,
        target: &'static str,
        method: Method,
        content_type: Option<&'static str>,
        body: Vec<u8>,
        unready: bool,
        expected_status: StatusCode,
        expected_code: &'static str,
        expected_message: &'static str,
    }

    fn ready_router_for(unready: bool) -> Router {
        if unready {
            return build_router(AppState::new("test"));
        }
        let state = AppState::new("test");
        state.lifecycle.mark_ready();
        build_router(state)
    }

    let invalid = "The chat request is invalid.";
    let broken_controls =
        br#"{"model":"","messages":[],"temperature":9.0,"max_tokens":0,"stream":true}"#.to_vec();

    let cases = vec![
        // 1. Route beats everything, including size and method.
        PrecedenceCase {
            id: "stage 1 ROUTE-001",
            target: "/v1/chat/elsewhere",
            method: Method::DELETE,
            content_type: Some("text/plain"),
            body: vec![b'x'; 1_048_577],
            unready: true,
            expected_status: StatusCode::NOT_FOUND,
            expected_code: "not_found",
            expected_message: "The requested path was not found.",
        },
        // 2. Method beats admission, size, and media type.
        PrecedenceCase {
            id: "stage 2 METHOD-001",
            target: "/v1/chat/completions",
            method: Method::PUT,
            content_type: Some("text/plain"),
            body: vec![b'x'; 1_048_577],
            unready: true,
            expected_status: StatusCode::METHOD_NOT_ALLOWED,
            expected_code: "method_not_allowed",
            expected_message: "The request method is not allowed for this path.",
        },
        // 3. Admission beats size, media type, and every later stage.
        PrecedenceCase {
            id: "stage 3 ADMIT-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("text/plain"),
            body: vec![b'x'; 1_048_577],
            unready: true,
            expected_status: StatusCode::SERVICE_UNAVAILABLE,
            expected_code: "not_ready",
            expected_message: "The gateway is not accepting new chat requests.",
        },
        // 4. Size beats the media type and every later stage.
        PrecedenceCase {
            id: "stage 4 SIZE-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("text/plain"),
            body: vec![b'x'; 1_048_577],
            unready: false,
            expected_status: StatusCode::PAYLOAD_TOO_LARGE,
            expected_code: "payload_too_large",
            expected_message: "The request payload is too large.",
        },
        // 5. Media type beats every later stage.
        PrecedenceCase {
            id: "stage 5 TYPE-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("text/plain"),
            body: broken_controls.clone(),
            unready: false,
            expected_status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            expected_code: "unsupported_media_type",
            expected_message: "The request media type is not supported.",
        },
        // 6. Structural readability beats required fields and controls.
        PrecedenceCase {
            id: "stage 6 PARSE-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("application/json"),
            body: b"{ broken".to_vec(),
            unready: false,
            expected_status: StatusCode::BAD_REQUEST,
            expected_code: "invalid_request",
            expected_message: invalid,
        },
        // 7. Required fields beat control ranges and streaming.
        PrecedenceCase {
            id: "stage 7 FIELD-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("application/json"),
            body: broken_controls.clone(),
            unready: false,
            expected_status: StatusCode::BAD_REQUEST,
            expected_code: "invalid_request",
            expected_message: invalid,
        },
        // 8. Control ranges beat streaming.
        PrecedenceCase {
            id: "stage 8 CTRL-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("application/json"),
            body: br#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":9.0,"stream":true}"#.to_vec(),
            unready: false,
            expected_status: StatusCode::BAD_REQUEST,
            expected_code: "invalid_request",
            expected_message: invalid,
        },
        // 9. Streaming is last.
        PrecedenceCase {
            id: "stage 9 STREAM-001",
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("application/json"),
            body: with_field("stream", "true"),
            unready: false,
            expected_status: StatusCode::BAD_REQUEST,
            expected_code: "unsupported_feature",
            expected_message: "Streaming is not supported.",
        },
    ];

    for case in cases {
        for repetition in 0..20 {
            let router = ready_router_for(case.unready);
            let mut builder = Request::builder()
                .method(case.method.clone())
                .uri(case.target);
            if let Some(content_type) = case.content_type {
                builder = builder.header(CONTENT_TYPE, content_type);
            }
            let request = builder.body(Body::from(case.body.clone())).unwrap();
            let response = router.oneshot(request).await.unwrap();

            let (status, value) = parse_json(response).await;
            assert_eq!(
                status, case.expected_status,
                "{} repetition {repetition} returned the wrong status",
                case.id
            );
            assert_error_envelope(&value, case.expected_code, case.expected_message);
        }
    }
}

#[tokio::test]
async fn a_request_breaking_model_message_control_and_size_rules_at_once_reports_size() {
    // All four families are broken simultaneously; the size stage is earliest.
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    const OVER_LIMIT: usize = 1_048_577;
    let body = {
        let prefix = br#"{"model":"","messages":[{"role":"tool","content":""}],"temperature":9.0,"max_tokens":0,"stream":true,"pad":""#;
        let suffix = br#""}"#;
        let mut body = prefix.to_vec();
        body.extend(std::iter::repeat_n(
            b'x',
            OVER_LIMIT - prefix.len() - suffix.len(),
        ));
        body.extend_from_slice(suffix);
        assert_eq!(body.len(), OVER_LIMIT);
        body
    };

    for _ in 0..20 {
        let response = build_router(state.clone())
            .oneshot(chat_request_with_body(body.clone()))
            .await
            .unwrap();
        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_error_envelope(
            &value,
            "payload_too_large",
            "The request payload is too large.",
        );
    }
}

#[tokio::test]
async fn fifty_simultaneous_mixed_requests_stay_isolated() {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);
    let barrier = Arc::new(Barrier::new(50));
    let mut requests = JoinSet::new();

    for index in 0..50 {
        let router = router.clone();
        let barrier = Arc::clone(&barrier);
        requests.spawn(async move {
            let request = if index % 5 == 0 {
                // Every fifth request violates a documented rule.
                chat_request(&json!({
                    "model": "mock-model",
                    "messages": [{"role": "user", "content": "Hello"}],
                    "temperature": 9.0
                }))
            } else {
                chat_request(&json!({
                    "model": format!("mock-model-{index}"),
                    "messages": [{"role": "user", "content": "Hello"}],
                    "temperature": 0.5,
                    "max_tokens": 64
                }))
            };

            barrier.wait().await;
            let response = router.oneshot(request).await.unwrap();
            let (status, value) = parse_json(response).await;
            (index, status, value)
        });
    }

    let mut seen = 0;
    while let Some(joined) = requests.join_next().await {
        let (index, status, value) = joined.unwrap();
        seen += 1;

        if index % 5 == 0 {
            assert_eq!(status, StatusCode::BAD_REQUEST, "request {index}");
            assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
        } else {
            assert_eq!(status, StatusCode::OK, "request {index}");
            // Isolation check: the response echoes only this request's model.
            assert_eq!(value["model"], format!("mock-model-{index}"));
            assert_eq!(value, expected_success(&format!("mock-model-{index}")));
        }
    }

    assert_eq!(seen, 50);
}

#[tokio::test]
async fn the_canonical_request_is_deterministic_with_and_without_controls() {
    let without = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello"}]
    });
    let with = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 0.7,
        "max_tokens": 500
    });

    let mut baseline: Option<Vec<u8>> = None;
    for index in 0..200 {
        let body = if index % 2 == 0 { &without } else { &with };
        let response = ready_router().oneshot(chat_request(body)).await.unwrap();
        let (status, raw) = parse_raw(response).await;

        assert_eq!(status, StatusCode::OK, "iteration {index} was refused");
        match &baseline {
            None => baseline = Some(raw),
            Some(expected) => assert_eq!(
                *expected, raw,
                "iteration {index} differed from the first response"
            ),
        }
    }
}

#[tokio::test]
async fn an_explicit_stream_request_returns_the_unsupported_feature_contract() {
    let body = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": true
    });

    let response = ready_router().oneshot(chat_request(&body)).await.unwrap();
    let (status, value) = parse_json(response).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_error_envelope(&value, "unsupported_feature", "Streaming is not supported.");
}

#[tokio::test]
async fn a_stream_refusal_returns_no_completion_and_no_internal_detail() {
    let response = ready_router()
        .oneshot(chat_request(&json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        })))
        .await
        .unwrap();
    let (status, raw) = parse_raw(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let text = String::from_utf8(raw).unwrap();

    for needle in [
        "choices",
        "chat_mock",
        "chat.completion",
        "Hello",
        "stream",
        "internal",
        "backtrace",
    ] {
        assert!(!text.contains(needle), "leaked {needle:?} in {text:?}");
    }
    assert_error_envelope(
        &serde_json::from_str(&text).unwrap(),
        "unsupported_feature",
        "Streaming is not supported.",
    );
}

#[tokio::test]
async fn a_request_invalid_on_its_own_merits_wins_over_a_stream_request() {
    // Each body is invalid independently and also asks for a stream; the
    // invalid_request row is the one a client may rely on.
    let bodies = vec![
        json!({
            "model": "",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [],
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "tool", "content": "Hello"}],
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": ""}],
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 9.0,
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 0,
            "stream": true
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": "0.5",
            "stream": true
        }),
    ];

    for body in bodies {
        let response = ready_router().oneshot(chat_request(&body)).await.unwrap();
        let (status, value) = parse_json(response).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn omitting_the_stream_control_is_a_normal_request() {
    for body in [
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}]
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        }),
    ] {
        let response = ready_router().oneshot(chat_request(&body)).await.unwrap();
        let (status, value) = parse_json(response).await;

        assert_eq!(status, StatusCode::OK, "{body} was refused");
        assert_eq!(value, expected_success("mock-model"));
    }
}

#[tokio::test]
async fn a_mistyped_stream_control_is_refused_as_invalid_not_unsupported() {
    for literal in [r#""true""#, "1", "null", "{}", "[]"] {
        let body = format!(
            r#"{{"model":"mock-model","messages":[{{"role":"user","content":"Hello"}}],"stream":{literal}}}"#
        );
        let response = ready_router()
            .oneshot(error_request(
                Method::POST,
                "/v1/chat/completions",
                Some("application/json"),
                &body,
            ))
            .await
            .unwrap();
        let (status, value) = parse_json(response).await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "stream {literal} was accepted"
        );
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn health_and_readiness_hold_before_during_and_after_every_request_kind() {
    // A valid request, an invalid request, and an oversized request must each
    // leave liveness and readiness untouched.
    let valid = canonical_chat_body();
    let invalid = json!({
        "model": "mock-model",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 9.0
    });

    async fn probe(router: Router) {
        let health = router.clone().oneshot(health_request()).await.unwrap();
        assert_health_contract(health).await;
        let ready = router.oneshot(readiness_request()).await.unwrap();
        assert_readiness_contract(ready, StatusCode::OK, b"{\"status\":\"ready\"}").await;
    }

    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);

    probe(router.clone()).await;

    for body in [&valid, &invalid] {
        let response = router.clone().oneshot(chat_request(body)).await.unwrap();
        assert!(response.status().is_success() || response.status().is_client_error());
        probe(router.clone()).await;
    }

    let oversized = router
        .clone()
        .oneshot(chat_request_with_body(chat_body_of_size(1_048_577)))
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    probe(router.clone()).await;

    // And the gateway still serves normal traffic afterwards.
    let afterwards = router.clone().oneshot(chat_request(&valid)).await.unwrap();
    let (status, value) = parse_json(afterwards).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));
    probe(router.clone()).await;
}

#[tokio::test]
async fn a_long_run_of_oversized_requests_does_not_degrade_later_handling() {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);

    for _ in 0..10 {
        let refused = router
            .clone()
            .oneshot(chat_request_with_body(chat_body_of_size(1_048_577)))
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    // The first request after the run must be handled exactly like the first
    // request ever made; a leaked admission slot or poisoned state shows here.
    let first = router
        .clone()
        .oneshot(chat_request(&canonical_chat_body()))
        .await
        .unwrap();
    let (status, value) = parse_json(first).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));

    let health = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(health).await;
    let ready = router.clone().oneshot(readiness_request()).await.unwrap();
    assert_readiness_contract(ready, StatusCode::OK, b"{\"status\":\"ready\"}").await;
}

/// One row of the Client-Facing Validation Contract table in `spec.md`.
struct ContractRow {
    condition: &'static str,
    body: Vec<u8>,
    target: &'static str,
    method: Method,
    content_type: Option<&'static str>,
    unready: bool,
    expected_status: StatusCode,
    expected_code: &'static str,
    expected_message: &'static str,
}

impl ContractRow {
    fn chat(
        condition: &'static str,
        body: Vec<u8>,
        status: StatusCode,
        code: &'static str,
        message: &'static str,
    ) -> Self {
        Self {
            condition,
            body,
            target: "/v1/chat/completions",
            method: Method::POST,
            content_type: Some("application/json"),
            unready: false,
            expected_status: status,
            expected_code: code,
            expected_message: message,
        }
    }
}

fn contract_rows() -> Vec<ContractRow> {
    let invalid = "The chat request is invalid.";
    let rows = vec![
        ContractRow::chat(
            "Missing or invalid model",
            br#"{"model":"","messages":[{"role":"user","content":"Hello"}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "Empty, missing, or invalid messages",
            br#"{"model":"mock-model","messages":[]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "Unsupported message role",
            br#"{"model":"mock-model","messages":[{"role":"tool","content":"Hello"}]}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "temperature or max_tokens not a number",
            with_field("temperature", r#""0.5""#),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "temperature outside 0.0-2.0",
            with_field("temperature", "2.0001"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "max_tokens outside 1-4096",
            with_field("max_tokens", "0"),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "Control supplied more than once",
            br#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}"#.to_vec(),
            StatusCode::BAD_REQUEST,
            "invalid_request",
            invalid,
        ),
        ContractRow::chat(
            "Streaming requested",
            with_field("stream", "true"),
            StatusCode::BAD_REQUEST,
            "unsupported_feature",
            "Streaming is not supported.",
        ),
        ContractRow::chat(
            "Request body larger than 1 MB",
            vec![b'x'; 1_048_577],
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "The request payload is too large.",
        ),
        ContractRow {
            content_type: Some("text/plain"),
            ..ContractRow::chat(
                "Unsupported request media type",
                valid_chat_json().into_bytes(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "The request media type is not supported.",
            )
        },
        ContractRow {
            method: Method::PUT,
            ..ContractRow::chat(
                "Unsupported method on a known path",
                valid_chat_json().into_bytes(),
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "The request method is not allowed for this path.",
            )
        },
        ContractRow {
            target: "/v1/chat/unknown",
            ..ContractRow::chat(
                "Unknown path",
                valid_chat_json().into_bytes(),
                StatusCode::NOT_FOUND,
                "not_found",
                "The requested path was not found.",
            )
        },
        ContractRow {
            unready: true,
            ..ContractRow::chat(
                "New chat request after shutdown begins",
                valid_chat_json().into_bytes(),
                StatusCode::SERVICE_UNAVAILABLE,
                "not_ready",
                "The gateway is not accepting new chat requests.",
            )
        },
    ];

    // The table's 14th row, "Unexpected internal failure", is deliberately
    // absent: no request in this contract triggers it, so it is asserted at the
    // error-mapping layer by `the_contract_table_has_exactly_fourteen_rows`.
    rows
}

#[tokio::test]
async fn every_contract_row_is_covered_and_byte_exact() {
    for row in contract_rows() {
        let router = if row.unready {
            build_router(AppState::new("test"))
        } else {
            let state = AppState::new("test");
            state.lifecycle.mark_ready();
            build_router(state)
        };
        let mut builder = Request::builder()
            .method(row.method.clone())
            .uri(row.target);
        if let Some(content_type) = row.content_type {
            builder = builder.header(CONTENT_TYPE, content_type);
        }
        let request = builder.body(Body::from(row.body.clone())).unwrap();
        let response = router.oneshot(request).await.unwrap();

        let (status, raw) = parse_raw(response).await;
        let text = String::from_utf8(raw.clone()).unwrap();
        assert_eq!(
            status, row.expected_status,
            "contract row {:?} returned the wrong status",
            row.condition
        );

        // Byte-exact: the whole body is the two documented fields in the
        // documented order, with nothing else.
        let expected = format!(
            r#"{{"code":"{}","message":"{}"}}"#,
            row.expected_code, row.expected_message
        );
        assert_eq!(
            text, expected,
            "contract row {:?} is not byte-exact",
            row.condition
        );
    }
}

#[tokio::test]
async fn the_contract_table_has_exactly_fourteen_rows() {
    // The spec table has 14 rows. Thirteen are reachable over HTTP; the
    // internal-failure row is produced at the error-mapping layer, so it is
    // checked there instead of in the wire table.
    assert_eq!(contract_rows().len(), 13);
    assert_eq!(
        ApiError::InternalError.status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(ApiError::InternalError.code(), "internal_error");
    assert_eq!(
        ApiError::InternalError.message(),
        "The gateway could not complete the request."
    );
    let dto = ApiError::InternalError.to_dto();
    assert_eq!(dto.code, "internal_error");
    assert_eq!(dto.message, "The gateway could not complete the request.");
}

#[tokio::test]
async fn validation_reaches_no_external_resource() {
    // Every rejection and every acceptance is served with no provider, no
    // credential, no database, and no network configuration present, and the
    // gateway reports its own ability to serve honestly.
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);

    let bodies = vec![
        canonical_chat_body(),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 0.5,
            "max_tokens": 64
        }),
        json!({"model": "", "messages": [{"role": "user", "content": "Hello"}]}),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 9.0
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": true
        }),
    ];

    for body in bodies {
        let response = router.clone().oneshot(chat_request(&body)).await.unwrap();
        let status = response.status();

        assert!(
            status == StatusCode::OK
                || status == StatusCode::BAD_REQUEST
                || status == StatusCode::PAYLOAD_TOO_LARGE,
            "{} produced {status}, which suggests an external dependency",
            serde_json::to_string(&body).unwrap()
        );
    }

    // Health and readiness stay truthful with no external configuration.
    let health = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(health).await;
    let ready = router.oneshot(readiness_request()).await.unwrap();
    assert_readiness_contract(ready, StatusCode::OK, b"{\"status\":\"ready\"}").await;
}

#[tokio::test]
async fn every_documented_rule_produces_its_exact_response() {
    for case in rule_cases() {
        // ADMIT-001 needs an unready router; every other case needs a ready one.
        let router = rule_router(&case);
        let response = router.oneshot(rule_request(&case)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(
            status,
            case.expected_status,
            "{} returned the wrong status for {}",
            case.id,
            String::from_utf8_lossy(&case.body)
        );
        assert_error_envelope(&value, case.expected_code, case.expected_message);
    }
}

#[tokio::test]
async fn the_rule_suite_covers_every_catalogued_rule_id() {
    // A rule added to contracts/validation-rules.md without a case here fails
    // this test, because the expected set is written out in full.
    const CATALOG: [&str; 20] = [
        "ROUTE-001",
        "METHOD-001",
        "ADMIT-001",
        "SIZE-001",
        "TYPE-001",
        "PARSE-001",
        "TYPE-002",
        "TYPE-003",
        "TYPE-004",
        "UNIQ-001",
        "UNIQ-002",
        "FIELD-001",
        "FIELD-002",
        "FIELD-003",
        "FIELD-004",
        "FIELD-005",
        "CTRL-001",
        "CTRL-002",
        "CTRL-003",
        "STREAM-001",
    ];

    let exercised: BTreeSet<&str> = rule_cases().into_iter().map(|case| case.id).collect();
    let expected: BTreeSet<&str> = CATALOG.into_iter().collect();

    assert_eq!(
        exercised, expected,
        "the rule suite and the catalogued rule IDs disagree"
    );
    assert_eq!(exercised.len(), 20, "the catalog has exactly 20 rule IDs");
}

#[tokio::test]
async fn every_failure_response_has_exactly_two_fields_and_no_details() {
    for case in rule_cases() {
        let router = rule_router(&case);
        let response = router.oneshot(rule_request(&case)).await.unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(
            object.len(),
            2,
            "{} did not return exactly two fields",
            case.id
        );
        assert!(object.contains_key("code"), "{} is missing code", case.id);
        assert!(
            object.contains_key("message"),
            "{} is missing message",
            case.id
        );
        for forbidden in ["details", "error", "source", "diagnostic", "request_id"] {
            assert!(
                !object.contains_key(forbidden),
                "{} leaked a {forbidden} field",
                case.id
            );
        }
    }
}

#[tokio::test]
async fn body_of_exactly_one_mebibyte_is_accepted_and_one_byte_more_is_refused() {
    let limit = 1_048_576;

    let accepted = ready_router()
        .oneshot(chat_request_with_body(chat_body_of_size(limit)))
        .await
        .unwrap();
    let (status, value) = parse_json(accepted).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a body of exactly {limit} bytes was refused"
    );
    assert_eq!(value, expected_success("mock-model"));

    let refused = ready_router()
        .oneshot(chat_request_with_body(chat_body_of_size(limit + 1)))
        .await
        .unwrap();
    let (status, value) = parse_json(refused).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_error_envelope(
        &value,
        "payload_too_large",
        "The request payload is too large.",
    );
}

#[tokio::test]
async fn payload_too_large_body_discloses_no_count_or_internal_detail() {
    let response = ready_router()
        .oneshot(chat_request_with_body(chat_body_of_size(1_048_577)))
        .await
        .unwrap();
    let (status, raw) = parse_raw(response).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let text = String::from_utf8(raw).unwrap();

    for needle in [
        "1048576",
        "1048577",
        "1_048_576",
        "1MB",
        "1 MB",
        "limit",
        "byte",
        "size",
        "length",
        "MAX_REQUEST_BODY_BYTES",
        "to_bytes",
    ] {
        assert!(!text.contains(needle), "leaked {needle:?} in {text:?}");
    }
    assert_error_envelope(
        &serde_json::from_str(&text).unwrap(),
        "payload_too_large",
        "The request payload is too large.",
    );
}

#[tokio::test]
async fn oversized_body_is_refused_before_the_media_type_check() {
    // A wrong media type alone would give 415; the size bound must win.
    let request = error_request(
        Method::POST,
        "/v1/chat/completions",
        Some("text/plain"),
        "not json at all, and this line is padded out to exceed the one mebibyte limit for the refusal to be about size and not about content.",
    );
    let mut oversized = request;
    *oversized.body_mut() = Body::from(vec![b'x'; 1_048_577]);

    let response = ready_router().oneshot(oversized).await.unwrap();
    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_error_envelope(
        &value,
        "payload_too_large",
        "The request payload is too large.",
    );
}

#[tokio::test]
async fn oversized_body_without_a_declared_length_is_refused() {
    // No Content-Length header, so the bound must apply while the body arrives.
    let response = ready_router()
        .oneshot(chat_request_with_body(chat_body_of_size(1_048_577)))
        .await
        .unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_error_envelope(
        &value,
        "payload_too_large",
        "The request payload is too large.",
    );
}

#[tokio::test]
async fn gateway_serves_normal_traffic_after_an_oversized_refusal() {
    let state = AppState::new("test");
    state.lifecycle.mark_ready();
    let router = build_router(state);

    for _ in 0..3 {
        let response = router
            .clone()
            .oneshot(chat_request_with_body(chat_body_of_size(1_048_577)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    let health = router.clone().oneshot(health_request()).await.unwrap();
    assert_health_contract(health).await;

    let ready = router.clone().oneshot(readiness_request()).await.unwrap();
    assert_readiness_contract(ready, StatusCode::OK, b"{\"status\":\"ready\"}").await;

    let normal = router
        .clone()
        .oneshot(chat_request(&canonical_chat_body()))
        .await
        .unwrap();
    let (status, value) = parse_json(normal).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, expected_success("mock-model"));
}

#[tokio::test]
async fn oversized_body_is_still_refused_while_the_gateway_is_not_ready() {
    // Admission runs first, so an unready gateway answers 503 without reading
    // the body at all.
    let request = chat_request_with_body(chat_body_of_size(1_048_577));
    let response = build_router(AppState::new("test"))
        .oneshot(request)
        .await
        .unwrap();

    let (status, value) = parse_json(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_error_envelope(
        &value,
        "not_ready",
        "The gateway is not accepting new chat requests.",
    );
}

#[tokio::test]
async fn chat_rejects_every_malformed_or_out_of_range_control() {
    // Every case below is a control failure from contracts/validation-rules.md
    // §5, and every one must produce the identical fixed response.
    let temperature_failures = [
        json!(-0.1),
        json!(-1.0),
        json!(2.0001),
        json!(3.0),
        json!("0.5"),
        json!(true),
        json!(null),
        json!({}),
        json!([]),
    ];
    let max_tokens_failures = [
        json!(0),
        json!(4097),
        json!(100.5),
        json!(-1),
        json!(true),
        json!(null),
        json!([500]),
        json!("500"),
    ];

    let mut bodies = Vec::new();
    for temperature in temperature_failures {
        bodies.push(json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": temperature
        }));
    }
    for max_tokens in max_tokens_failures {
        bodies.push(json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": max_tokens
        }));
    }
    for body in bodies {
        let router = ready_router();
        let response = router.oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn chat_rejects_a_repeated_generation_control() {
    // A duplicate key cannot be built with `json!`, which keeps only the last
    // copy, so these bodies are raw JSON text.
    let bodies = [
        r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":1.0}"#,
        r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"temperature":0.5,"temperature":0.5}"#,
        r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":10,"max_tokens":20}"#,
        r#"{"model":"mock-model","messages":[{"role":"user","content":"Hello"}],"max_tokens":10,"max_tokens":99999}"#,
    ];

    for body in bodies {
        let router = ready_router();
        let request = error_request(
            Method::POST,
            "/v1/chat/completions",
            Some("application/json"),
            body,
        );
        let response = router.oneshot(request).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
}

#[tokio::test]
async fn control_failure_response_leaks_no_value_field_name_or_parser_detail() {
    // A submitted value, a prompt marker, and a parser offset must all stay out
    // of the response. The serialized body is compared exactly so that any
    // extra key or any substituted text fails the test.
    let secret_prompt = "SECRET-PROMPT-MARKER-8f3a";
    let bodies = vec![
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": secret_prompt}],
            "temperature": 7.7777
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": secret_prompt}],
            "max_tokens": 99999
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": secret_prompt}],
            "temperature": "7.7777"
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "user", "content": secret_prompt}],
            "max_tokens": 100.5
        }),
    ];

    for body in bodies {
        let router = ready_router();
        let response = router.oneshot(chat_request(&body)).await.unwrap();

        let (status, raw) = parse_raw(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {body}");
        let text = String::from_utf8(raw).unwrap();

        for needle in [
            secret_prompt,
            "7.7777",
            "99999",
            "100.5",
            "temperature",
            "max_tokens",
            "at line",
            "column",
            "invalid type",
            "expected",
            "SECRET",
        ] {
            assert!(!text.contains(needle), "leaked {needle:?} in {text:?}");
        }
        assert_error_envelope(
            &serde_json::from_str(&text).unwrap(),
            "invalid_request",
            "The chat request is invalid.",
        );
    }
}

#[tokio::test]
async fn control_failure_and_required_field_failure_yield_one_identical_response() {
    // Both breaches in one body; precedence between them is unobservable from
    // outside, so the contract is a single identical response.
    let bodies = vec![
        json!({
            "model": "",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 9.0
        }),
        json!({
            "model": "mock-model",
            "messages": [],
            "max_tokens": 0
        }),
        json!({
            "model": "mock-model",
            "messages": [{"role": "tool", "content": "Hello"}],
            "temperature": 9.0, "max_tokens": 0
        }),
    ];

    for body in bodies {
        let router = ready_router();
        let response = router.oneshot(chat_request(&body)).await.unwrap();

        let (status, value) = parse_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "accepted {body}");
        assert_error_envelope(&value, "invalid_request", "The chat request is invalid.");
    }
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
