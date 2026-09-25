use std::{
    net::{SocketAddr, TcpListener as StdTcpListener},
    sync::Arc,
    time::{Duration, Instant},
};

use ai_gateway::{
    api::{
        health::{health, readiness},
        middleware::admit_chat,
        server::{run, serve_router_with_shutdown, serve_with_shutdown},
    },
    application::{AppState, lifecycle::GatewayPhase},
    config::ServerConfig,
};
use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    middleware,
    routing::{get, post},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{Notify, oneshot},
    time::timeout,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_BUDGET: Duration = Duration::from_millis(250);
const FORCED_DEADLINE: Duration = Duration::from_millis(75);
const FORCED_COMPLETION_TIMEOUT: Duration = Duration::from_millis(500);
const NOT_READY_BODY: &[u8] =
    br#"{"code":"not_ready","message":"The gateway is not accepting new chat requests."}"#;

#[derive(Clone)]
struct HeldSignals {
    admitted: Arc<Notify>,
}

async fn held_chat_handler(
    Extension(signals): Extension<HeldSignals>,
    body: Body,
) -> axum::http::StatusCode {
    signals.admitted.notify_one();
    let _ = to_bytes(body, usize::MAX).await;
    axum::http::StatusCode::NO_CONTENT
}

fn held_chat_router(state: AppState, signals: HeldSignals) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route(
            "/v1/chat/completions",
            post(held_chat_handler)
                .layer(middleware::from_fn_with_state(state.clone(), admit_chat)),
        )
        .layer(Extension(signals))
        .with_state(state)
}

async fn get_path(address: SocketAddr, path: &str) -> Vec<u8> {
    timeout(REQUEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    })
    .await
    .unwrap_or_else(|_| panic!("HTTP request timed out for {path}"))
}

async fn post_chat(address: SocketAddr, body: &str) -> Vec<u8> {
    timeout(REQUEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let request = format!(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    })
    .await
    .expect("chat request timed out")
}

async fn open_partial_chat(address: SocketAddr) -> TcpStream {
    timeout(REQUEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(
                b"POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 64\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        stream
            .write_all(br#"{"model":"mock-model","messages":["#)
            .await
            .unwrap();
        stream
    })
    .await
    .expect("partial chat request timed out")
}

fn assert_json_response(response: &[u8], status_line: &str, expected_body: &[u8]) {
    let response = std::str::from_utf8(response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with(status_line));
    assert!(
        headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("content-type: application/json"))
    );
    assert_eq!(body.as_bytes(), expected_body);
}

async fn wait_for_ready(state: &AppState) {
    timeout(STARTUP_TIMEOUT, async {
        while !state.lifecycle.is_ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("server did not become ready");
}

async fn wait_for_shutdown(state: &AppState) {
    timeout(SHUTDOWN_TIMEOUT, async {
        while state.lifecycle.phase() != GatewayPhase::ShuttingDown {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("server did not enter shutdown");
}

async fn assert_socket_released(address: SocketAddr) {
    let listener = timeout(SHUTDOWN_TIMEOUT, async {
        loop {
            match StdTcpListener::bind(address) {
                Ok(listener) => break listener,
                Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                    tokio::task::yield_now().await;
                }
                Err(error) => panic!("failed to check released socket at {address}: {error}"),
            }
        }
    })
    .await
    .expect("listening socket was not released");
    drop(listener);
}

async fn get_health(address: SocketAddr) -> Vec<u8> {
    timeout(REQUEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    })
    .await
    .expect("health request timed out")
}

fn assert_health_response(response: &[u8]) {
    let response = std::str::from_utf8(response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("\r\ncontent-type: application/json\r\n")
    );
    assert_eq!(body, r#"{"status":"ok"}"#);
}

#[tokio::test]
async fn bound_server_remains_available_until_injected_shutdown() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let state = AppState::new("test");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let server = tokio::spawn(serve_with_shutdown(state.clone(), listener, async move {
        let _ = shutdown_receiver.await;
    }));

    timeout(STARTUP_TIMEOUT, async {
        while !state.lifecycle.is_ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("server did not become ready");
    assert!(!server.is_finished());

    for _ in 0..3 {
        assert_health_response(&get_health(address).await);
        assert!(!server.is_finished());
    }

    shutdown_sender.send(()).unwrap();
    let server_result = timeout(SHUTDOWN_TIMEOUT, server)
        .await
        .expect("server did not stop after shutdown")
        .unwrap();

    assert!(server_result.is_ok());
    assert_eq!(
        state.lifecycle.phase(),
        ai_gateway::application::lifecycle::GatewayPhase::Stopped
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bound_server_handles_one_hundred_health_and_readiness_requests() {
    let listener = StdTcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let state = AppState::new("test");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let server = tokio::spawn(serve_with_shutdown(state.clone(), listener, async move {
        let _ = shutdown_receiver.await;
    }));
    wait_for_ready(&state).await;

    let mut health_within_budget = 0;
    for _ in 0..100 {
        let started = Instant::now();
        let response = get_path(address, "/health").await;
        assert_json_response(&response, "HTTP/1.1 200 OK\r\n", br#"{"status":"ok"}"#);
        if started.elapsed() < REQUEST_BUDGET {
            health_within_budget += 1;
        }
    }

    let mut readiness_within_budget = 0;
    for _ in 0..100 {
        let started = Instant::now();
        let response = get_path(address, "/ready").await;
        assert_json_response(&response, "HTTP/1.1 200 OK\r\n", br#"{"status":"ready"}"#);
        if started.elapsed() < REQUEST_BUDGET {
            readiness_within_budget += 1;
        }
    }

    assert!(
        health_within_budget >= 95,
        "only {health_within_budget} health requests completed within 250ms"
    );
    assert!(
        readiness_within_budget >= 95,
        "only {readiness_within_budget} readiness requests completed within 250ms"
    );

    shutdown_sender.send(()).unwrap();
    let server_result = timeout(SHUTDOWN_TIMEOUT, server)
        .await
        .expect("server did not stop after request checks")
        .unwrap();
    assert!(server_result.is_ok(), "server returned {server_result:?}");
    assert_socket_released(address).await;
}

#[tokio::test]
async fn occupied_port_startup_failure_includes_bind_address_and_port() {
    let occupied_listener = StdTcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = occupied_listener.local_addr().unwrap();
    let config = ServerConfig {
        host: address.ip(),
        port: address.port(),
    };

    let result = timeout(
        STARTUP_TIMEOUT,
        run(config, AppState::new("test"), std::future::pending::<()>()),
    )
    .await
    .expect("occupied-port startup did not fail promptly");
    let error = result.expect_err("occupied-port startup unexpectedly succeeded");
    let diagnostics = format!("{error:#}");

    assert!(diagnostics.to_ascii_lowercase().contains("bind"));
    assert!(diagnostics.contains(&address.ip().to_string()));
    assert!(diagnostics.contains(&address.port().to_string()));
    assert!(diagnostics.contains("failed to bind HTTP listener"));

    drop(occupied_listener);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_drains_an_admitted_partial_chat_request() {
    let listener = StdTcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let state = AppState::new("test");
    let signals = HeldSignals {
        admitted: Arc::new(Notify::new()),
    };
    let router = held_chat_router(state.clone(), signals.clone());
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let server = tokio::spawn(serve_router_with_shutdown(
        state.clone(),
        listener,
        router,
        async move {
            let _ = shutdown_receiver.await;
        },
        SHUTDOWN_TIMEOUT,
    ));
    wait_for_ready(&state).await;

    let held_client = open_partial_chat(address).await;
    timeout(REQUEST_TIMEOUT, signals.admitted.notified())
        .await
        .expect("held chat handler was not admitted");

    shutdown_sender.send(()).unwrap();
    wait_for_shutdown(&state).await;

    assert_json_response(
        &get_path(address, "/health").await,
        "HTTP/1.1 200 OK\r\n",
        br#"{"status":"ok"}"#,
    );
    assert_json_response(
        &get_path(address, "/ready").await,
        "HTTP/1.1 503 Service Unavailable\r\n",
        br#"{"status":"not_ready"}"#,
    );
    assert_json_response(
        &post_chat(address, r#"{"model":"mock-model","messages":[]}"#).await,
        "HTTP/1.1 503 Service Unavailable\r\n",
        NOT_READY_BODY,
    );

    drop(held_client);
    timeout(SHUTDOWN_TIMEOUT, state.lifecycle.wait_for_zero())
        .await
        .expect("dropping the held client did not release admission");

    let server_result = timeout(SHUTDOWN_TIMEOUT, server)
        .await
        .expect("server did not finish after admission drained")
        .unwrap();
    assert!(server_result.is_ok(), "server returned {server_result:?}");
    assert_eq!(state.lifecycle.phase(), GatewayPhase::Stopped);
    assert_socket_released(address).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forced_shutdown_deadline_returns_contextual_error_and_releases_socket() {
    let listener = StdTcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let state = AppState::new("test");
    let signals = HeldSignals {
        admitted: Arc::new(Notify::new()),
    };
    let router = held_chat_router(state.clone(), signals.clone());
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let server = tokio::spawn(serve_router_with_shutdown(
        state.clone(),
        listener,
        router,
        async move {
            let _ = shutdown_receiver.await;
        },
        FORCED_DEADLINE,
    ));
    wait_for_ready(&state).await;

    let held_client = open_partial_chat(address).await;
    timeout(REQUEST_TIMEOUT, signals.admitted.notified())
        .await
        .expect("held chat handler was not admitted");

    let shutdown_started = Instant::now();
    shutdown_sender.send(()).unwrap();
    let server_result = timeout(FORCED_COMPLETION_TIMEOUT, server)
        .await
        .expect("forced shutdown did not return near its deadline")
        .unwrap();
    let error = server_result.expect_err("forced shutdown unexpectedly succeeded");
    let diagnostics = format!("{error:#}").to_ascii_lowercase();

    assert!(diagnostics.contains("shutdown"));
    assert!(diagnostics.contains("deadline") || diagnostics.contains("timed out"));
    assert!(shutdown_started.elapsed() < FORCED_COMPLETION_TIMEOUT);

    drop(held_client);
    assert_socket_released(address).await;
}
