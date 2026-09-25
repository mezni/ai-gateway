use std::{
    env,
    net::{IpAddr, Ipv4Addr},
};

use thiserror::Error;

const HOST_VARIABLE: &str = "AI_GATEWAY_HOST";
const PORT_VARIABLE: &str = "AI_GATEWAY_PORT";
const DEFAULT_PORT: u16 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ServerConfigError> {
        let host = match env::var(HOST_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ServerConfigError::InvalidHostEncoding);
            }
        };
        let port = match env::var(PORT_VARIABLE) {
            Ok(value) => Some(value),
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ServerConfigError::InvalidPortEncoding);
            }
        };

        Self::from_values(host.as_deref(), port.as_deref())
    }

    fn from_values(host: Option<&str>, port: Option<&str>) -> Result<Self, ServerConfigError> {
        let host = match host {
            Some(value) if value.trim().is_empty() => return Err(ServerConfigError::EmptyHost),
            Some(value) => value
                .parse::<IpAddr>()
                .map_err(|_| ServerConfigError::InvalidHost {
                    value: value.to_owned(),
                })?,
            None => IpAddr::V4(Ipv4Addr::LOCALHOST),
        };
        let port = match port {
            Some(value) if value.trim().is_empty() => return Err(ServerConfigError::EmptyPort),
            Some(value) => {
                let port = value
                    .parse::<u16>()
                    .map_err(|_| ServerConfigError::InvalidPort {
                        value: value.to_owned(),
                    })?;
                if port == 0 {
                    return Err(ServerConfigError::InvalidPort {
                        value: value.to_owned(),
                    });
                }
                port
            }
            None => DEFAULT_PORT,
        };

        Ok(Self { host, port })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServerConfigError {
    #[error("{HOST_VARIABLE} is present but empty; expected a valid IPv4 or IPv6 address")]
    EmptyHost,
    #[error("{HOST_VARIABLE} must be a valid IPv4 or IPv6 address: {value}")]
    InvalidHost { value: String },
    #[error("{HOST_VARIABLE} must contain valid Unicode")]
    InvalidHostEncoding,
    #[error("{PORT_VARIABLE} is present but empty; expected a non-zero integer from 1 to 65535")]
    EmptyPort,
    #[error("{PORT_VARIABLE} must be a non-zero integer from 1 to 65535: {value}")]
    InvalidPort { value: String },
    #[error("{PORT_VARIABLE} must contain valid Unicode")]
    InvalidPortEncoding,
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::*;

    #[test]
    fn from_values_returns_defaults_when_values_are_absent() {
        let config = ServerConfig::from_values(None, None).unwrap();

        assert_eq!(
            config,
            ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3000,
            }
        );
    }

    #[test]
    fn from_values_accepts_ipv4_host_override() {
        let config = ServerConfig::from_values(Some("192.0.2.10"), None).unwrap();

        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10)));
    }

    #[test]
    fn from_values_accepts_ipv6_host_override() {
        let config = ServerConfig::from_values(Some("2001:db8::1"), None).unwrap();

        assert_eq!(
            config.host,
            IpAddr::V6("2001:db8::1".parse::<Ipv6Addr>().unwrap())
        );
    }

    #[test]
    fn from_values_accepts_nonzero_port_override() {
        let config = ServerConfig::from_values(None, Some("65535")).unwrap();

        assert_eq!(config.port, 65535);
    }

    #[test]
    fn from_values_rejects_empty_host_with_clear_error() {
        let error = ServerConfig::from_values(Some(""), None).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_HOST"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_malformed_host_with_clear_error() {
        let error = ServerConfig::from_values(Some("localhost"), None).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_HOST"));
        assert!(error.to_string().contains("IPv4 or IPv6"));
    }

    #[test]
    fn from_values_rejects_empty_port_with_clear_error() {
        let error = ServerConfig::from_values(None, Some("")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PORT"));
        assert!(error.to_string().contains("empty"));
    }

    #[test]
    fn from_values_rejects_zero_port_with_clear_error() {
        let error = ServerConfig::from_values(None, Some("0")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PORT"));
        assert!(error.to_string().contains("non-zero"));
    }

    #[test]
    fn from_values_rejects_malformed_port_with_clear_error() {
        let error = ServerConfig::from_values(None, Some("not-a-port")).unwrap_err();

        assert!(error.to_string().contains("AI_GATEWAY_PORT"));
        assert!(error.to_string().contains("non-zero"));
    }

    #[test]
    fn from_values_parses_deterministically_without_process_environment() {
        let first = ServerConfig::from_values(Some("::1"), Some("8080")).unwrap();
        let second = ServerConfig::from_values(Some("::1"), Some("8080")).unwrap();

        assert_eq!(first, second);
    }
}
