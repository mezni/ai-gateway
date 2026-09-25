//! Application layer: orchestration and the [`AppState`] composition root.
//!
//! Owns orchestration logic and application-level state. May depend on
//! `domain` and `config`, never on `api` or `infrastructure`.
//! See `specs/002-layered-architecture/contracts/layout.md`.

use std::sync::Arc;

use chat::MockChatCompletionService;
use lifecycle::GatewayLifecycle;

pub mod chat;
pub mod lifecycle;

/// Application state shared across the gateway (composition root).
#[derive(Debug, Clone)]
pub struct AppState {
    version: &'static str,
    pub lifecycle: Arc<GatewayLifecycle>,
    pub chat_service: MockChatCompletionService,
}

impl AppState {
    /// Constructs the application state with the given crate version.
    pub fn new(version: &'static str) -> Self {
        Self {
            version,
            lifecycle: Arc::new(GatewayLifecycle::new()),
            chat_service: MockChatCompletionService::new(),
        }
    }

    /// Returns the banner line printed at startup.
    pub fn banner(&self) -> String {
        format!("AI Gateway v{}", self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_matches_expected_output() {
        let state = AppState::new("0.1.0");
        assert_eq!(state.banner(), "AI Gateway v0.1.0");
    }

    #[test]
    fn new_starts_in_initializing_phase_and_is_not_ready() {
        let state = AppState::new("0.1.0");

        assert_eq!(
            state.lifecycle.phase(),
            lifecycle::GatewayPhase::Initializing
        );
        assert!(!state.lifecycle.is_ready());
    }
}
