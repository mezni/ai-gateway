use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{api::error::ApiError, application::AppState};

pub async fn admit_chat(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(guard) = state.lifecycle.try_admit_chat() else {
        return ApiError::NotReady.into_response();
    };
    let response = next.run(request).await;
    drop(guard);
    response
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

    use super::admit_chat;
    use crate::application::AppState;

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
