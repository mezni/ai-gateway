use std::env;

use thiserror::Error;

const PROVIDER_VARIABLE: &str = "AI_GATEWAY_PROVIDER";
const DEADLINE_VARIABLE: &str = "AI_GATEWAY_PROVIDER_TIMEOUT_MS";

/// The provider selected when `AI_GATEWAY_PROVIDER` is unset.
///
/// Deterministic, so the gateway serves traffic with no configuration at all
/// (FR-005).
pub const DEFAULT_PROVIDER_ID: &str = "deterministic";

/// Deadline applied when `AI_GATEWAY_PROVIDER_TIMEOUT_MS` is unset.
pub const DEFAULT_DEADLINE_MS: u64 = 30_000;

/// Largest accepted deadline. Bounds how long a request can be held open, so a
/// mistyped value cannot pin connections indefinitely.
pub const MAX_DEADLINE_MS: u64 = 300_000;

/// Which provider serves traffic, and how long a single call may take.
///
/// This type validates *syntax only*. Whether `provider_id` names a provider
/// that actually exists is decided later, by the registry resolving the
/// selection at startup; the config layer must not depend on `infrastructure`,
/// so it cannot consult the registry itself (layout rule 4, FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    /// Identifier of the selected provider.
    pub provider_id: String,
    /// Per-call deadline in milliseconds. Always strictly positive.
    pub deadline_ms: u64,
}

impl Default for ProviderConfig {
    /// The configuration a gateway runs with when nothing is set: the built-in
    /// deterministic provider under the default deadline.
    fn default() -> Self {
        Self {
            provider_id: DEFAULT_PROVIDER_ID.to_owned(),
            deadline_ms: DEFAULT_DEADLINE_MS,
        }
    }
}

impl ProviderConfig {
    /// Reads provider configuration from the process environment.
    pub fn from_env() -> Result<Self, ProviderConfigError> {
        let provider = match env::var(PROVIDER_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ProviderConfigError::InvalidProviderEncoding);
            }
        };
        let deadline = match env::var(DEADLINE_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ProviderConfigError::InvalidDeadlineEncoding);
            }
        };

        Self::from_values(provider.as_deref(), deadline.as_deref())
    }

    /// Builds a configuration from already-read values.
    ///
    /// An absent value falls back to the documented default. A value that is
    /// present but unusable is an error, never a silent fallback: a mistyped
    /// deadline that quietly became 30 seconds would be indistinguishable from
    /// the value the operator intended (FR-011, FR-028).
    fn from_values(
        provider: Option<&str>,
        deadline: Option<&str>,
    ) -> Result<Self, ProviderConfigError> {
        let provider_id = match provider {
            Some(value) if value.trim().is_empty() => {
                return Err(ProviderConfigError::EmptyProvider);
            }
            Some(value) if !is_valid_provider_id(value) => {
                return Err(ProviderConfigError::InvalidProvider {
                    value: value.to_owned(),
                });
            }
            Some(value) => value.to_owned(),
            None => DEFAULT_PROVIDER_ID.to_owned(),
        };

        let deadline_ms = match deadline {
            Some(value) if value.trim().is_empty() => {
                return Err(ProviderConfigError::EmptyDeadline);
            }
            Some(value) => {
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| ProviderConfigError::InvalidDeadline {
                        value: value.to_owned(),
                    })?;
                if parsed == 0 {
                    return Err(ProviderConfigError::InvalidDeadline {
                        value: value.to_owned(),
                    });
                }
                if parsed > MAX_DEADLINE_MS {
                    return Err(ProviderConfigError::DeadlineTooLarge {
                        value: parsed,
                    });
                }
                parsed
            }
            None => DEFAULT_DEADLINE_MS,
        };

        Ok(Self {
            provider_id,
            deadline_ms,
        })
    }
}

/// A provider id is lowercase ASCII letters, digits, and single separators.
///
/// Restricting the alphabet keeps ids usable as an env value and in a log line
/// without quoting rules, and keeps them comparable to the registry keys that
/// implement `LlmProvider::id`.
fn is_valid_provider_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderConfigError {
    #[error("{PROVIDER_VARIABLE} is present but empty; expected a provider identifier")]
    EmptyProvider,
    #[error(
        "{PROVIDER_VARIABLE} must be lowercase ASCII letters, digits, '-' or '_': {value}"
    )]
    InvalidProvider { value: String },
    #[error("{PROVIDER_VARIABLE} must contain valid Unicode")]
    InvalidProviderEncoding,
    #[error("{DEADLINE_VARIABLE} is present but empty; expected milliseconds from 1 to {MAX_DEADLINE_MS}")]
    EmptyDeadline,
    #[error("{DEADLINE_VARIABLE} must be a positive integer no greater than {MAX_DEADLINE_MS} milliseconds: {value}")]
    InvalidDeadline { value: String },
    #[error("{DEADLINE_VARIABLE} must not exceed {MAX_DEADLINE_MS} milliseconds: {value}")]
    DeadlineTooLarge { value: u64 },
    #[error("{DEADLINE_VARIABLE} must contain valid Unicode")]
    InvalidDeadlineEncoding,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_values_returns_defaults_when_values_are_absent() {
        let config = ProviderConfig::from_values(None, None).unwrap();

        assert_eq!(config, ProviderConfig::default());
    }

    #[test]
    fn from_values_accepts_explicit_valid_values() {
        let config = ProviderConfig::from_values(Some("openrouter"), Some("5000")).unwrap();

        assert_eq!(
            config,
            ProviderConfig {
                provider_id: "openrouter".to_owned(),
                deadline_ms: 5000,
            }
        );
    }

    #[test]
    fn from_values_accepts_known_id_shape_even_when_registry_cannot_yet_check_it() {
        // Syntax is validated here; existence is the registry's job at startup.
        let config = ProviderConfig::from_values(Some("a-b_c9"), None).unwrap();

        assert_eq!(config.provider_id, "a-b_c9");
    }

    #[test]
    fn from_values_rejects_empty_provider_with_clear_error() {
        let error = ProviderConfig::from_values(Some(""), None).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PROVIDER"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_provider_with_invalid_characters() {
        for value in ["Deterministic", "open router", "open/router", "opén"] {
            let error = ProviderConfig::from_values(Some(value), None).unwrap_err();

            assert!(
                error.to_string().contains("AI_GATEWAY_PROVIDER"),
                "value {value:?} should name the variable"
            );
        }
    }

    #[test]
    fn from_values_rejects_empty_deadline_with_clear_error() {
        let error = ProviderConfig::from_values(None, Some("")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PROVIDER_TIMEOUT_MS"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_malformed_deadline_with_clear_error() {
        let error = ProviderConfig::from_values(None, Some("not-a-number")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PROVIDER_TIMEOUT_MS"));
    }

    #[test]
    fn from_values_rejects_negative_deadline() {
        let error = ProviderConfig::from_values(None, Some("-1")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PROVIDER_TIMEOUT_MS"));
    }

    #[test]
    fn from_values_rejects_zero_deadline() {
        let error = ProviderConfig::from_values(None, Some("0")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PROVIDER_TIMEOUT_MS"));
        assert!(error.to_string().contains("positive"));
    }

    #[test]
    fn from_values_rejects_deadline_above_the_cap() {
        let error = ProviderConfig::from_values(None, Some("300001")).unwrap_err();

        assert!(error.to_string().contains("300000"));
    }

    #[test]
    fn from_values_accepts_the_exact_cap() {
        let config = ProviderConfig::from_values(None, Some("300000")).unwrap();

        assert_eq!(config.deadline_ms, MAX_DEADLINE_MS);
    }

    #[test]
    fn from_values_never_silently_falls_back_when_a_value_is_present() {
        // The regression this guards: a present-but-broken value must not become
        // the default, or a mistyped deadline is invisible to the operator.
        let error = ProviderConfig::from_values(Some("deterministic"), Some("0")).unwrap_err();

        assert!(matches!(error, ProviderConfigError::InvalidDeadline { .. }));
    }

    #[test]
    fn from_values_parses_deterministically_without_process_environment() {
        let first = ProviderConfig::from_values(Some("deterministic"), Some("1500")).unwrap();
        let second = ProviderConfig::from_values(Some("deterministic"), Some("1500")).unwrap();

        assert_eq!(first, second);
    }
}
