//! OpenRouter provider configuration: whether the adapter is selectable, which
//! endpoint it talks to, and which environment variable holds its credential.
//!
//! The credential itself is never named or stored here; only the variable's
//! name is. The adapter reads that variable at startup (Phase 4 provider
//! contract, obligation 1), so the secret never crosses a layer boundary.

use std::env;

use thiserror::Error;

const ENABLED_VARIABLE: &str = "AI_GATEWAY_OPENROUTER_ENABLED";
const BASE_URL_VARIABLE: &str = "AI_GATEWAY_OPENROUTER_BASE_URL";
const API_KEY_ENV_VARIABLE: &str = "AI_GATEWAY_OPENROUTER_API_KEY_ENV";

/// Whether the OpenRouter adapter is selectable when no override is set.
pub const DEFAULT_ENABLED: bool = true;

/// Endpoint the OpenRouter adapter talks to when no override is set.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Name of the environment variable holding the API key when no override is
/// set. The variable holds the secret; this constant only names it.
pub const DEFAULT_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

/// Settings that steer the OpenRouter adapter.
///
/// As with [`super::provider::ProviderConfig`](super::provider::ProviderConfig),
/// this type validates syntax only; whether the named variable is actually set
/// is the adapter's concern at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRouterConfig {
    /// Whether the adapter may be selected.
    pub enabled: bool,
    /// Chat-completion API root, e.g. `https://openrouter.ai/api/v1`.
    pub base_url: String,
    /// Name of the environment variable that holds the API key.
    pub api_key_env: String,
}

impl Default for OpenRouterConfig {
    /// The configuration an operator gets when nothing is set: the live
    /// OpenRouter endpoint, enabled, reading the documented variable.
    fn default() -> Self {
        Self {
            enabled: DEFAULT_ENABLED,
            base_url: DEFAULT_BASE_URL.to_owned(),
            api_key_env: DEFAULT_API_KEY_ENV.to_owned(),
        }
    }
}

impl OpenRouterConfig {
    /// Reads OpenRouter provider configuration from the process environment.
    pub fn from_env() -> Result<Self, OpenRouterConfigError> {
        let enabled = match env::var(ENABLED_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(OpenRouterConfigError::InvalidEnabledEncoding);
            }
        };
        let base_url = match env::var(BASE_URL_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(OpenRouterConfigError::InvalidBaseUrlEncoding);
            }
        };
        let api_key_env = match env::var(API_KEY_ENV_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(OpenRouterConfigError::InvalidApiKeyEnvEncoding);
            }
        };

        Self::from_values(enabled.as_deref(), base_url.as_deref(), api_key_env.as_deref())
    }

    /// Builds a configuration from already-read values.
    ///
    /// An absent value falls back to the documented default. A value that is
    /// present but unusable is an error, never a silent fallback.
    pub fn from_values(
        enabled: Option<&str>,
        base_url: Option<&str>,
        api_key_env: Option<&str>,
    ) -> Result<Self, OpenRouterConfigError> {
        let enabled = match enabled {
            Some(value) if value.trim().is_empty() => {
                return Err(OpenRouterConfigError::EmptyEnabled);
            }
            Some(value) => match value {
                "true" => true,
                "false" => false,
                other => {
                    return Err(OpenRouterConfigError::InvalidEnabled {
                        value: other.to_owned(),
                    });
                }
            },
            None => DEFAULT_ENABLED,
        };

        let base_url = match base_url {
            Some(value) if value.trim().is_empty() => return Err(OpenRouterConfigError::EmptyBaseUrl),
            Some(value) => {
                let trimmed = value.trim();
                if !is_http_endpoint(trimmed) {
                    return Err(OpenRouterConfigError::InvalidBaseUrl {
                        value: value.to_owned(),
                    });
                }
                trimmed.to_owned()
            }
            None => DEFAULT_BASE_URL.to_owned(),
        };

        let api_key_env = match api_key_env {
            Some(value) if value.trim().is_empty() => {
                return Err(OpenRouterConfigError::EmptyApiKeyEnv);
            }
            Some(value) => {
                let trimmed = value.trim();
                if !is_valid_env_name(trimmed) {
                    return Err(OpenRouterConfigError::InvalidApiKeyEnv {
                        value: value.to_owned(),
                    });
                }
                trimmed.to_owned()
            }
            None => DEFAULT_API_KEY_ENV.to_owned(),
        };

        Ok(Self {
            enabled,
            base_url,
            api_key_env,
        })
    }
}

/// A usable endpoint is an `http://` or `https://` URL with a non-empty
/// remainder and no whitespace anywhere.
fn is_http_endpoint(value: &str) -> bool {
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"));
    rest.is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace))
}

/// A usable variable name is a POSIX identifier: ASCII letters, digits, and
/// underscores, and not starting with a digit.
fn is_valid_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpenRouterConfigError {
    #[error(
        "{ENABLED_VARIABLE} is present but empty; expected \"true\" or \"false\""
    )]
    EmptyEnabled,
    #[error(
        "{ENABLED_VARIABLE} must be \"true\" or \"false\": {value}"
    )]
    InvalidEnabled { value: String },
    #[error("{ENABLED_VARIABLE} must contain valid Unicode")]
    InvalidEnabledEncoding,
    #[error(
        "{BASE_URL_VARIABLE} is present but empty; expected an http:// or https:// endpoint"
    )]
    EmptyBaseUrl,
    #[error(
        "{BASE_URL_VARIABLE} must be an http:// or https:// URL: {value}"
    )]
    InvalidBaseUrl { value: String },
    #[error("{BASE_URL_VARIABLE} must contain valid Unicode")]
    InvalidBaseUrlEncoding,
    #[error(
        "{API_KEY_ENV_VARIABLE} is present but empty; expected an environment variable name"
    )]
    EmptyApiKeyEnv,
    #[error(
        "{API_KEY_ENV_VARIABLE} must be a valid environment variable name: {value}"
    )]
    InvalidApiKeyEnv { value: String },
    #[error("{API_KEY_ENV_VARIABLE} must contain valid Unicode")]
    InvalidApiKeyEnvEncoding,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_values_returns_defaults_when_values_are_absent() {
        let config = OpenRouterConfig::from_values(None, None, None).unwrap();

        assert_eq!(config, OpenRouterConfig::default());
        assert_eq!(config.enabled, true);
        assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(config.api_key_env, "OPENROUTER_API_KEY");
    }

    #[test]
    fn from_values_accepts_explicit_valid_values() {
        let config = OpenRouterConfig::from_values(
            Some("false"),
            Some("https://openrouter.example/api/v1"),
            Some("MY_OPENROUTER_KEY"),
        )
        .unwrap();

        assert_eq!(
            config,
            OpenRouterConfig {
                enabled: false,
                base_url: "https://openrouter.example/api/v1".to_owned(),
                api_key_env: "MY_OPENROUTER_KEY".to_owned(),
            }
        );
    }

    #[test]
    fn from_values_trims_and_normalizes_the_endpoint() {
        let config = OpenRouterConfig::from_values(None, Some("  https://openrouter.ai/api/v1  "), None)
            .unwrap();

        assert_eq!(config.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn from_values_accepts_a_local_http_endpoint() {
        // Local test endpoints are legitimate values, so the alphabet must not
        // assume production hostnames.
        let config = OpenRouterConfig::from_values(None, Some("http://127.0.0.1:4010/v1"), None)
            .unwrap();

        assert_eq!(config.base_url, "http://127.0.0.1:4010/v1");
    }

    #[test]
    fn from_values_rejects_empty_enabled_with_clear_error() {
        let error = OpenRouterConfig::from_values(Some(""), None, None).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_OPENROUTER_ENABLED"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_non_boolean_enabled_with_clear_error() {
        for value in ["yes", "1", "TRUE", "true ", ""] {
            let error = OpenRouterConfig::from_values(Some(value), None, None).unwrap_err();

            assert!(
                error.to_string().contains("AI_GATEWAY_OPENROUTER_ENABLED"),
                "value {value:?} should name the variable"
            );
        }
    }

    #[test]
    fn from_values_rejects_empty_base_url_with_clear_error() {
        let error = OpenRouterConfig::from_values(None, Some(""), None).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_OPENROUTER_BASE_URL"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_a_base_url_without_a_supported_scheme() {
        for value in ["ftp://openrouter.ai", "openrouter.ai", "https:/openrouter.ai", "https://"] {
            let error = OpenRouterConfig::from_values(None, Some(value), None).unwrap_err();

            assert!(
                error.to_string().contains("AI_GATEWAY_OPENROUTER_BASE_URL"),
                "value {value:?} should name the variable"
            );
        }
    }

    #[test]
    fn from_values_rejects_a_base_url_with_internal_whitespace() {
        let error = OpenRouterConfig::from_values(None, Some("https://open router.ai/v1"), None)
            .unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_OPENROUTER_BASE_URL"));
    }

    #[test]
    fn from_values_rejects_empty_api_key_env_with_clear_error() {
        let error = OpenRouterConfig::from_values(None, None, Some("")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_OPENROUTER_API_KEY_ENV"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_invalid_environment_variable_names() {
        for value in ["9KEY", "MY-KEY", "MY KEY", "MY.KEY"] {
            let error = OpenRouterConfig::from_values(None, None, Some(value)).unwrap_err();

            assert!(
                error.to_string().contains("AI_GATEWAY_OPENROUTER_API_KEY_ENV"),
                "value {value:?} should name the variable"
            );
        }
    }

    #[test]
    fn from_values_parses_deterministically_without_process_environment() {
        let first = OpenRouterConfig::from_values(Some("true"), Some("https://or.example/v1"), Some("K"))
            .unwrap();
        let second = OpenRouterConfig::from_values(
            Some("true"),
            Some("https://or.example/v1"),
            Some("K"),
        )
        .unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn a_present_but_broken_value_never_becomes_the_default() {
        // The same regression the selection config guards: an operator typo in
        // one variable must not silently re-default the other values.
        for (enabled, base_url, api_key_env) in [
            (Some("maybe"), None, None),
            (None, Some("ftp://or.example"), None),
            (None, None, Some("9BAD")),
        ] {
            assert!(
                OpenRouterConfig::from_values(enabled, base_url, api_key_env).is_err(),
                "{enabled:?} {base_url:?} {api_key_env:?} should be refused"
            );
        }
    }
}
