use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{api::error::ApiError, application::AppState};

/// Largest accepted request body, whole, in bytes.
///
/// The bound is inclusive: a body of exactly this many bytes is admitted and
/// one byte more is refused.
pub const MAX_REQUEST_BODY_BYTES: usize = 1_048_576;

pub async fn admit_chat(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(guard) = state.lifecycle.try_admit_chat() else {
        return ApiError::NotReady.into_response();
    };
    let response = next.run(request).await;
    drop(guard);
    response
}

/// Bounds the request body before any media-type check or JSON parsing.
///
/// A declared `Content-Length` above the limit is refused without the body
/// being read at all, so an oversized request costs no buffering. Otherwise the
/// body is read under the limit and a body that still exceeds it while arriving
/// is refused, which is the case for a request that declared no length.
///
/// This must run after admission and before the `Json` extractor, because that
/// extractor checks the media type before it buffers anything. Leaving the limit
/// in `DefaultBodyLimit` would make an oversized body with a wrong media type
/// return 415 instead of the documented 413.
pub async fn bound_chat_body(request: Request, next: Next) -> Response {
    if declared_length(&request).is_some_and(|declared| declared > MAX_REQUEST_BODY_BYTES) {
        return ApiError::PayloadTooLarge.into_response();
    }

    let (parts, body) = request.into_parts();
    let limit = MAX_REQUEST_BODY_BYTES;
    let Ok(bytes) = to_bytes(body, limit).await else {
        return ApiError::PayloadTooLarge.into_response();
    };

    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}

/// The declared `Content-Length`, when the client sent a usable one.
///
/// A header that is absent or unparsable yields `None`, so the body is bounded
/// while it arrives instead of being trusted.
fn declared_length(request: &Request) -> Option<usize> {
    request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::{Extension, Json, Request},
        http::{Method, StatusCode, header::CONTENT_TYPE},
        middleware,
        response::{IntoResponse, Response},
        routing::post,
    };
    use serde_json::{Value, json};
    use tokio::sync::Notify;
    use tower::ServiceExt;

    use super::{MAX_REQUEST_BODY_BYTES, admit_chat, bound_chat_body};
    use crate::application::AppState;

    fn bounded_chat_router(state: AppState) -> Router {
        Router::new()
            .route("/chat", post(test_chat_handler))
            .layer(middleware::from_fn(bound_chat_body))
            .layer(middleware::from_fn_with_state(state, admit_chat))
    }

    fn sized_request(content_type: Option<&str>, body: Vec<u8>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("/chat")
            .header(axum::http::header::CONTENT_LENGTH, body.len());
        if let Some(content_type) = content_type {
            builder = builder.header(CONTENT_TYPE, content_type);
        }
        builder.body(Body::from(body)).unwrap()
    }

    /// A body of exactly `total` bytes.
    ///
    /// For sizes large enough to hold it, the bytes form a valid JSON chat
    /// request whose message content is padded with `x`; smaller sizes are
    /// returned as filler, which is enough for a size-bound test.
    fn body_of_size(total: usize) -> Vec<u8> {
        const PREFIX: &[u8] = br#"{"model":"mock-model","messages":[{"role":"user","content":""#;
        const SUFFIX: &[u8] = br#""}]}"#;

        if total <= PREFIX.len() + SUFFIX.len() {
            return vec![b'x'; total];
        }

        let mut body = PREFIX.to_vec();
        body.extend(std::iter::repeat_n(
            b'x',
            total - PREFIX.len() - SUFFIX.len(),
        ));
        body.extend_from_slice(SUFFIX);
        body
    }

    async fn assert_payload_too_large(response: Response) {
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value,
            json!({
                "code": "payload_too_large",
                "message": "The request payload is too large.",
            })
        );
        assert_eq!(value.as_object().map(serde_json::Map::len), Some(2));
    }

    #[test]
    fn max_request_body_bytes_is_one_mebibyte() {
        assert_eq!(MAX_REQUEST_BODY_BYTES, 1_048_576);
    }

    #[test]
    fn body_of_size_helper_hits_the_requested_length() {
        for total in [1, 64, 1_000, 65_536, MAX_REQUEST_BODY_BYTES] {
            assert_eq!(body_of_size(total).len(), total);
        }
    }

    #[tokio::test]
    async fn body_of_exactly_the_limit_is_admitted() {
        let state = ready_state();
        let response = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("application/json"),
                body_of_size(MAX_REQUEST_BODY_BYTES),
            ))
            .await
            .unwrap();

        // The padded body is valid JSON, so it reaches the handler and is echoed
        // back as "forwarded" rather than refused with 413.
        assert_ne!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn declared_length_one_byte_over_the_limit_is_refused() {
        let state = ready_state();
        let response = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("application/json"),
                body_of_size(MAX_REQUEST_BODY_BYTES + 1),
            ))
            .await
            .unwrap();

        assert_payload_too_large(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn undeclared_length_over_the_limit_is_refused_while_arriving() {
        let state = ready_state();
        // No Content-Length, so the refusal must come from bounding the stream.
        let request = Request::builder()
            .method(Method::POST)
            .uri("/chat")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body_of_size(MAX_REQUEST_BODY_BYTES + 1)))
            .unwrap();
        let response = bounded_chat_router(state.clone())
            .oneshot(request)
            .await
            .unwrap();

        assert_payload_too_large(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn unparsable_content_length_does_not_grant_an_oversized_body() {
        let state = ready_state();
        let request = Request::builder()
            .method(Method::POST)
            .uri("/chat")
            .header(CONTENT_TYPE, "application/json")
            .header(axum::http::header::CONTENT_LENGTH, "not-a-number")
            .body(Body::from(body_of_size(MAX_REQUEST_BODY_BYTES + 1)))
            .unwrap();
        let response = bounded_chat_router(state.clone())
            .oneshot(request)
            .await
            .unwrap();

        assert_payload_too_large(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn over_limit_refusal_precedes_the_media_type_check() {
        let state = ready_state();
        let response = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("text/plain"),
                body_of_size(MAX_REQUEST_BODY_BYTES + 1),
            ))
            .await
            .unwrap();

        assert_payload_too_large(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn an_oversized_refusal_still_releases_the_admission_slot() {
        let state = ready_state();
        let response = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("application/json"),
                body_of_size(MAX_REQUEST_BODY_BYTES + 1),
            ))
            .await
            .unwrap();

        assert_payload_too_large(response).await;
        // A stalled admission slot would make graceful shutdown hang.
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .expect("the admission slot was not released after an oversized refusal");
    }

    #[tokio::test]
    async fn a_second_request_is_still_admitted_after_an_oversized_refusal() {
        let state = ready_state();
        let refused = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("application/json"),
                body_of_size(MAX_REQUEST_BODY_BYTES + 1),
            ))
            .await
            .unwrap();
        assert_payload_too_large(refused).await;

        let admitted = bounded_chat_router(state.clone())
            .oneshot(sized_request(
                Some("application/json"),
                body_of_size(MAX_REQUEST_BODY_BYTES),
            ))
            .await
            .unwrap();
        assert_ne!(admitted.status(), StatusCode::SERVICE_UNAVAILABLE);

        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[derive(Clone)]
    struct HeldSignals {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    async fn test_chat_handler(Json(_body): Json<Value>) -> impl IntoResponse {
        "forwarded"
    }

    async fn held_chat_handler(Extension(signals): Extension<HeldSignals>) -> impl IntoResponse {
        signals.started.notify_one();
        signals.release.notified().await;
        "released"
    }

    fn chat_router(state: AppState) -> Router {
        Router::new()
            .route("/chat", post(test_chat_handler))
            .layer(middleware::from_fn_with_state(state, admit_chat))
    }

    fn held_chat_router(state: AppState) -> Router {
        Router::new()
            .route("/chat", post(held_chat_handler))
            .layer(middleware::from_fn_with_state(state, admit_chat))
    }

    fn ready_state() -> AppState {
        let state = AppState::new("test");
        state.lifecycle.mark_ready();
        state
    }

    fn chat_request(body: &'static str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/chat")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap()
    }

    fn held_chat_request(signals: HeldSignals) -> Request<Body> {
        let mut request = chat_request("{}");
        request.extensions_mut().insert(signals);
        request
    }

    async fn assert_not_ready_response(response: Response) {
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value,
            json!({
                "code": "not_ready",
                "message": "The gateway is not accepting new chat requests.",
            })
        );
        assert_eq!(value.as_object().map(serde_json::Map::len), Some(2));
    }

    #[tokio::test]
    async fn ready_admits_and_forwards_chat_request() {
        let state = ready_state();
        let response = chat_router(state.clone())
            .oneshot(chat_request(r#"{"model":"mock-model","messages":[]}"#))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            b"forwarded"
        );
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn initializing_rejects_chat_with_not_ready_contract() {
        let state = AppState::new("test");
        let response = chat_router(state.clone())
            .oneshot(chat_request(r#"{"model":"mock-model","messages":[]}"#))
            .await
            .unwrap();

        assert_not_ready_response(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn shutting_down_rejects_chat_with_not_ready_contract() {
        let state = ready_state();
        state.lifecycle.begin_shutdown();
        let response = chat_router(state.clone())
            .oneshot(chat_request(r#"{"model":"mock-model","messages":[]}"#))
            .await
            .unwrap();

        assert_not_ready_response(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn stopped_rejects_chat_with_not_ready_contract() {
        let state = ready_state();
        state.lifecycle.begin_shutdown();
        state.lifecycle.mark_stopped();
        let response = chat_router(state.clone())
            .oneshot(chat_request(r#"{"model":"mock-model","messages":[]}"#))
            .await
            .unwrap();

        assert_not_ready_response(response).await;
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn body_extraction_failure_releases_admission() {
        let state = ready_state();
        let response = chat_router(state.clone())
            .oneshot(chat_request("{"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        tokio::time::timeout(Duration::from_secs(1), state.lifecycle.wait_for_zero())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cancelling_held_downstream_future_releases_admission_and_notifies_waiter() {
        let state = ready_state();
        let signals = HeldSignals {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        };
        let request = held_chat_request(signals.clone());
        let task = tokio::spawn(held_chat_router(state.clone()).oneshot(request));

        tokio::time::timeout(Duration::from_secs(1), signals.started.notified())
            .await
            .expect("held handler did not start");

        let waiter_started = Arc::new(Notify::new());
        let waiter_lifecycle = state.lifecycle.clone();
        let waiter_started_for_task = Arc::clone(&waiter_started);
        let waiter = tokio::spawn(async move {
            waiter_started_for_task.notify_one();
            waiter_lifecycle.wait_for_zero().await;
        });
        tokio::time::timeout(Duration::from_secs(1), waiter_started.notified())
            .await
            .expect("waiter did not start");
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        task.abort();
        let task_result = task.await;
        assert!(task_result.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter was not notified")
            .expect("waiter task failed");
    }
}
