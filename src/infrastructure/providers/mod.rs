//! Provider implementations and the registry that selects between them.
//!
//! Each adapter in this module maps one external source of chat completions
//! onto the gateway's own [`LlmProvider`] port. The port is declared in the
//! application layer — `src/application/chat.rs` — because rule 4 of
//! `specs/002-layered-architecture/contracts/layout.md` forbids the application
//! layer from importing this one, and rule 9 allows this layer to import the
//! port types it implements.
//!
//! This layer may depend on `domain`, `config`, and `application` ports, never
//! on `api`, and never on application orchestration.

use std::collections::BTreeMap;
use std::sync::Arc;

use thiserror::Error;

use crate::application::chat::LlmProvider;
use crate::config::ProviderConfig;

mod deterministic;
mod openrouter;

pub use deterministic::DeterministicProvider;
pub use openrouter::{OpenRouterProvider, OpenRouterProviderError, OPENROUTER_ID};

/// One registered provider and whether it may currently be selected.
struct Registration {
    provider: Arc<dyn LlmProvider>,
    enabled: bool,
}

/// The set of providers the gateway can select between.
///
/// Ids come from [`LlmProvider::id`] and match the `AI_GATEWAY_PROVIDER`
/// vocabulary, so a selection made in configuration resolves to exactly one
/// adapter. Lookup is a `BTreeMap` read: it is not a source of request-path
/// latency.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Registration>,
}

impl ProviderRegistry {
    /// A registry holding only the built-in providers, all enabled.
    pub fn with_defaults() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(DeterministicProvider::new()));
        registry
    }

    /// Adds a provider, replacing any existing entry with the same id.
    pub fn register(&mut self, provider: Arc<dyn LlmProvider>) {
        let id = provider.id().to_owned();
        self.providers.insert(
            id,
            Registration {
                provider,
                enabled: true,
            },
        );
    }

    /// Enables or disables a registered provider.
    ///
    /// A disabled provider stays registered so the id remains a known one; only
    /// the selection is refused. Returns `false` when the id is not registered.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        match self.providers.get_mut(id) {
            Some(registration) => {
                registration.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// Ids of every provider an operator may currently select.
    pub fn enabled_ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|(_, registration)| registration.enabled)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Resolves a selection to the provider that will serve traffic.
    ///
    /// Both failure variants name the requested value, so a startup refusal
    /// tells the operator which value to correct (FR-007).
    pub fn resolve(&self, id: &str) -> Result<Arc<dyn LlmProvider>, ProviderResolutionError> {
        let registration = self
            .providers
            .get(id)
            .ok_or_else(|| ProviderResolutionError::UnknownProvider {
                requested: id.to_owned(),
            })?;

        if !registration.enabled {
            return Err(ProviderResolutionError::ProviderDisabled {
                requested: id.to_owned(),
            });
        }

        Ok(Arc::clone(&registration.provider))
    }

    /// Resolves the selection described by configuration.
    pub fn resolve_config(
        &self,
        config: &ProviderConfig,
    ) -> Result<Arc<dyn LlmProvider>, ProviderResolutionError> {
        self.resolve(&config.provider_id)
    }
}

/// Why a provider selection could not be served.
///
/// These are startup-time, operator-facing errors. They deliberately name the
/// requested value and nothing about the internals of the registry.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderResolutionError {
    #[error("no provider is registered under the name '{requested}'")]
    UnknownProvider { requested: String },
    #[error("the provider '{requested}' is registered but disabled")]
    ProviderDisabled { requested: String },
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::application::chat::{LlmProvider, ProviderFailure};
    use crate::domain::{ChatRequest, ChatResponse};

    use super::*;

    /// A named provider double, so a test can prove *which* provider a
    /// selection resolved to.
    struct Named(&'static str);

    #[async_trait]
    impl LlmProvider for Named {
        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
            Ok(ChatResponse::without_usage(self.0))
        }

        fn id(&self) -> &str {
            self.0
        }
    }

    /// A double whose id is fixed independently of what it returns, so two
    /// distinct instances can both claim the same registry key.
    struct FixedId {
        id: &'static str,
        body: &'static str,
    }

    #[async_trait]
    impl LlmProvider for FixedId {
        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderFailure> {
            Ok(ChatResponse::without_usage(self.body))
        }

        fn id(&self) -> &str {
            self.id
        }
    }

    #[test]
    fn default_registry_resolves_the_deterministic_provider() {
        let registry = ProviderRegistry::with_defaults();
        let config = ProviderConfig::default();

        let provider = registry.resolve_config(&config).unwrap();

        assert_eq!(provider.id(), "deterministic");
    }

    #[test]
    fn a_known_id_resolves_to_that_provider() {
        let mut registry = ProviderRegistry::with_defaults();
        registry.register(Arc::new(Named("alpha")));

        let provider = registry.resolve("alpha").unwrap();

        assert_eq!(provider.id(), "alpha");
    }

    #[test]
    fn an_unknown_id_fails_with_a_message_naming_the_requested_value() {
        let registry = ProviderRegistry::with_defaults();

        let error = registry.resolve("nope").err().expect("resolution should fail");

        assert!(error.to_string().contains("nope"), "got: {error}");
        assert!(matches!(
            error,
            ProviderResolutionError::UnknownProvider { .. }
        ));
    }

    #[test]
    fn a_disabled_id_fails_with_a_message_naming_the_requested_value() {
        let mut registry = ProviderRegistry::with_defaults();
        registry.register(Arc::new(Named("alpha")));
        registry.set_enabled("alpha", false);

        let error = registry.resolve("alpha").err().expect("resolution should fail");

        assert!(error.to_string().contains("alpha"), "got: {error}");
        assert!(matches!(
            error,
            ProviderResolutionError::ProviderDisabled { .. }
        ));
    }

    #[test]
    fn unknown_and_disabled_are_distinguishable_while_both_naming_the_value() {
        let mut registry = ProviderRegistry::with_defaults();
        registry.register(Arc::new(Named("alpha")));
        registry.set_enabled("alpha", false);

        let unknown = registry.resolve("ghost").err().expect("resolution should fail");
        let disabled = registry.resolve("alpha").err().expect("resolution should fail");

        assert_ne!(unknown.to_string(), disabled.to_string());
        assert!(unknown.to_string().contains("ghost"));
        assert!(disabled.to_string().contains("alpha"));
    }

    #[test]
    fn registering_a_different_id_adds_a_second_entry() {
        let mut registry = ProviderRegistry::with_defaults();
        registry.register(Arc::new(Named("alpha")));
        registry.register(Arc::new(Named("beta")));

        assert_eq!(registry.resolve("alpha").unwrap().id(), "alpha");
        assert_eq!(registry.resolve("beta").unwrap().id(), "beta");
    }

    #[test]
    fn registering_the_same_id_twice_replaces_the_provider() {
        let mut registry = ProviderRegistry::with_defaults();
        let first: Arc<dyn LlmProvider> = Arc::new(FixedId {
            id: "alpha",
            body: "first",
        });
        let second: Arc<dyn LlmProvider> = Arc::new(FixedId {
            id: "alpha",
            body: "second",
        });
        registry.register(Arc::clone(&first));
        registry.register(Arc::clone(&second));

        let resolved = registry.resolve("alpha").unwrap();

        // The later registration wins, and it is the very same allocation.
        assert!(Arc::ptr_eq(&resolved, &second));
        assert!(!Arc::ptr_eq(&resolved, &first));
    }

    #[test]
    fn resolution_returns_the_same_shared_instance_every_time() {
        // A registry holds `Arc`s, so two requests must not each build a client.
        let registry = ProviderRegistry::with_defaults();

        let first = registry.resolve("deterministic").unwrap();
        let second = registry.resolve("deterministic").unwrap();

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn enabled_ids_excludes_a_disabled_provider() {
        let mut registry = ProviderRegistry::with_defaults();
        registry.register(Arc::new(Named("alpha")));
        registry.set_enabled("alpha", false);

        let enabled = registry.enabled_ids();

        assert!(enabled.contains(&"deterministic"));
        assert!(!enabled.contains(&"alpha"));
    }

    #[test]
    fn set_enabled_reports_whether_the_id_was_registered() {
        let mut registry = ProviderRegistry::with_defaults();

        assert!(registry.set_enabled("deterministic", false));
        assert!(!registry.set_enabled("ghost", false));
    }
}
