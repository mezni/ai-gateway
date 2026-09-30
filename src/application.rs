//! Application layer: orchestration and the [`AppState`] composition root.
//!
//! Owns orchestration logic and application-level state. May depend on
//! `domain` and `config`, never on `api` or `infrastructure`.
//! See `specs/002-layered-architecture/contracts/layout.md`.

use std::sync::Arc;

use chat::{LlmProvider, MockChatCompletionService};
use crate::config::ProviderConfig;
use lifecycle::GatewayLifecycle;
use telemetry::ChatTelemetry;

pub mod chat;
pub mod lifecycle;
pub mod telemetry;

/// Application state shared across the gateway (composition root).
#[derive(Clone)]
pub struct AppState {
    version: &'static str,
    pub lifecycle: Arc<GatewayLifecycle>,
    /// The provider that serves chat completions.
    ///
    /// Held as a trait object so `main.rs` can inject any adapter without the
    /// application layer naming a concrete provider (layout rule 4).
    pub chat_service: Arc<dyn LlmProvider>,
    /// Resolved provider selection and the per-call deadline.
    pub provider_config: ProviderConfig,
    /// Per-request chat telemetry signals (counters plus the signal shape).
    pub telemetry: ChatTelemetry,
}

/// Hand-written so the trait object need not be `Debug`, which keeps
/// [`LlmProvider`] limited to the `Send + Sync` every adapter already has.
/// Reports the provider id rather than its internals: that is the part an
/// operator needs when reading a log line.
impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("version", &self.version)
            .field("provider", &self.chat_service.id())
            .field("deadline_ms", &self.provider_config.deadline_ms)
            .field("chat_requests", &self.telemetry.request_count())
            .field("lifecycle", &self.lifecycle)
            .finish()
    }
}

impl AppState {
    /// Constructs the application state with the given crate version.
    pub fn new(version: &'static str) -> Self {
        Self {
            version,
            lifecycle: Arc::new(GatewayLifecycle::new()),
            // Default provider lives in this layer so `new` stays callable
            // without infrastructure imports; `main.rs` overrides it with the
            // production adapter selected from configuration.
            chat_service: Arc::new(MockChatCompletionService::new()),
            provider_config: ProviderConfig::default(),
            telemetry: ChatTelemetry::new(),
        }
    }

    /// Constructs the application state with an explicit provider.
    pub fn new_with_provider(version: &'static str, provider: Arc<dyn LlmProvider>) -> Self {
        Self::new_with_provider_and_config(version, provider, ProviderConfig::default())
    }

    /// Constructs the application state with an explicit provider and the
    /// configuration that selected it. `main.rs` uses this; tests that only care
    /// about provider behaviour use [`Self::new_with_provider`].
    pub fn new_with_provider_and_config(
        version: &'static str,
        provider: Arc<dyn LlmProvider>,
        provider_config: ProviderConfig,
    ) -> Self {
        Self {
            version,
            lifecycle: Arc::new(GatewayLifecycle::new()),
            chat_service: provider,
            provider_config,
            telemetry: ChatTelemetry::new(),
        }
    }

    /// Id of the provider currently serving completions.
    pub fn provider_id(&self) -> &str {
        self.chat_service.id()
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
