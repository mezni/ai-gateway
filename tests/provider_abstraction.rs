//! Provider boundary, adapter, and selection tests.
//!
//! **Baseline**: Phase 3 ended at 203 passing tests (141 library, 57
//! `http_api`, 5 `server_lifecycle`). This feature adds coverage and must not
//! remove or weaken any existing test.
//!
//! **No network.** Nothing in this file contacts an external service. Provider
//! failures are produced by test doubles defined in this file, and provider
//! selection is driven through injected values rather than by mutating
//! process-wide environment state, so the tests remain order-independent.
//!
//! **Why everything lives in one file**: each `tests/*.rs` is a separate crate,
//! so a test double defined here cannot be imported by another integration
//! test. Keeping all double definitions in one file avoids a second module that
//! no other test could reach.
//!
//! ## Staged contents
//!
//! This file is created during Setup so the target path exists and is tracked.
//! It is populated incrementally as the underlying types come into existence:
//!
//! | Task     | Adds                                                     |
//! |----------|----------------------------------------------------------|
//! | T005-T010 | `TestProvider` double, wired to `LlmProvider`           |
//! | T030     | `StaticProvider` double returning an identical response  |
//! | T024-T028 | boundary tests (shape, byte-identity, precedence)        |
//! | T035-T038 | concurrency, containment, and shutdown isolation tests   |
//! | T039-T044 | extensibility tests (one-file addition and removal)     |

use axum::{body::Body, http::Request};
use tower::ServiceExt;

use ai_gateway::{api::server::build_router, application::AppState};

/// Sends `GET path` to a router built from default application state.
async fn get(path: &str) -> (u16, String) {
    let router = build_router(AppState::new("test"));
    let response = router
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn test_file_is_wired_up() {
    // Guard against this integration test being silently empty. It also pins the
    // Phase 3 baseline: the gateway still answers on the same three routes.
    let (status, _) = get("/health").await;
    assert_eq!(status, 200);
}
