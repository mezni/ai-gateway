use std::{
    future::{Future, IntoFuture},
    net::{SocketAddr, TcpListener as StdTcpListener},
    time::Duration,
};

use anyhow::{Context, anyhow};
use axum::{
    Router,
    body::Body,
    http::{
        HeaderValue, Method, StatusCode, Uri,
        header::{ALLOW, CONTENT_TYPE},
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{MethodFilter, MethodRouter, post},
};
use tokio::{sync::oneshot, time::Instant};

use crate::{
    api::{
        chat::chat_completions,
        error::ApiError,
        health::{health, readiness},
        middleware::admit_chat,
    },
    application::AppState,
    config::ServerConfig,
};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const HEALTH_PATH: &str = "/health";
const READY_PATH: &str = "/ready";
const CHAT_PATH: &str = "/v1/chat/completions";

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route(
            HEALTH_PATH,
            MethodRouter::new()
                .on(MethodFilter::GET, health)
                .on(MethodFilter::HEAD, head_not_allowed),
        )
        .route(
            READY_PATH,
            MethodRouter::new()
                .on(MethodFilter::GET, readiness)
                .on(MethodFilter::HEAD, head_not_allowed),
        )
        .route(
            CHAT_PATH,
            MethodRouter::new()
                .on(
                    MethodFilter::POST,
                    post(chat_completions)
                        .layer(middleware::from_fn_with_state(state.clone(), admit_chat)),
                )
                .on(MethodFilter::HEAD, head_not_allowed),
        )
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .with_state(state)
}

fn allowed_methods(path: &str) -> Option<&'static str> {
    match path {
        HEALTH_PATH | READY_PATH => Some("GET"),
        CHAT_PATH => Some("POST"),
        _ => None,
    }
}

fn body_suppressed(status: StatusCode, allow: Option<&'static str>) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(allow) = allow {
        headers.insert(ALLOW, HeaderValue::from_static(allow));
    }
    response
}

async fn method_not_allowed(uri: Uri) -> Response {
    match allowed_methods(uri.path()) {
        Some(allow) => {
            let mut response = ApiError::MethodNotAllowed.into_response();
            response
                .headers_mut()
                .insert(ALLOW, HeaderValue::from_static(allow));
            response
        }
        None => ApiError::NotFound.into_response(),
    }
}

async fn head_not_allowed(uri: Uri) -> Response {
    body_suppressed(StatusCode::METHOD_NOT_ALLOWED, allowed_methods(uri.path()))
}

async fn not_found(method: Method) -> Response {
    if method == Method::HEAD {
        return body_suppressed(StatusCode::NOT_FOUND, None);
    }
    ApiError::NotFound.into_response()
}

pub async fn run(
    config: ServerConfig,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let address = SocketAddr::new(config.host, config.port);
    let listener = StdTcpListener::bind(address)
        .with_context(|| format!("failed to bind HTTP listener at {address}"))?;
    serve_with_shutdown(state, listener, shutdown).await
}

pub async fn serve_with_shutdown(
    state: AppState,
    listener: StdTcpListener,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let router = build_router(state.clone());
    serve_router_with_shutdown(state, listener, router, shutdown, SHUTDOWN_TIMEOUT).await
}

pub async fn serve_router_with_shutdown(
    state: AppState,
    listener: StdTcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
    shutdown_timeout: Duration,
) -> anyhow::Result<()> {
    listener
        .set_nonblocking(true)
        .context("failed to configure the HTTP listener as nonblocking")?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .context("failed to register the HTTP listener with Tokio")?;
    state.lifecycle.mark_ready();

    let (graceful_shutdown_sender, graceful_shutdown_receiver) = oneshot::channel();
    let mut server = tokio::spawn(
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = graceful_shutdown_receiver.await;
            })
            .into_future(),
    );
    tokio::pin!(shutdown);

    tokio::select! {
        result = &mut server => {
            state.lifecycle.mark_stopped();
            return resolve_server_result(result);
        }
        _ = &mut shutdown => {}
    }

    let deadline = Instant::now() + shutdown_timeout;
    state.lifecycle.begin_shutdown();
    let drain = state.lifecycle.wait_for_zero();
    tokio::pin!(drain);
    let drain_result = tokio::select! {
        result = &mut server => {
            state.lifecycle.mark_stopped();
            return resolve_server_result(result);
        }
        result = tokio::time::timeout_at(deadline, &mut drain) => result,
    };
    if drain_result.is_err() {
        server.abort();
        let _ = server.await;
        state.lifecycle.mark_stopped();
        return Err(anyhow!(
            "graceful shutdown deadline of {shutdown_timeout:?} exceeded while waiting for admitted requests"
        ));
    }

    let _ = graceful_shutdown_sender.send(());
    match tokio::time::timeout_at(deadline, &mut server).await {
        Ok(result) => {
            state.lifecycle.mark_stopped();
            resolve_server_result(result)
        }
        Err(_) => {
            server.abort();
            let _ = server.await;
            state.lifecycle.mark_stopped();
            Err(anyhow!(
                "graceful shutdown deadline of {shutdown_timeout:?} exceeded while closing the HTTP server"
            ))
        }
    }
}

fn resolve_server_result(
    result: Result<std::io::Result<()>, tokio::task::JoinError>,
) -> anyhow::Result<()> {
    match result {
        Ok(result) => result.context("HTTP server failed"),
        Err(error) => Err(anyhow!("HTTP server task failed: {error}")),
    }
}
