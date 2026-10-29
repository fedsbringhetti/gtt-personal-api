use std::net::SocketAddr;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub bind_addr: SocketAddr,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid value {value:?} for {var}: {reason}")]
    Invalid {
        var: &'static str,
        value: String,
        reason: String,
    },
}

impl Config {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the configuration through `lookup`, so tests do not touch the real environment.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let bind_addr = parse_or_default(&lookup, "BIND_ADDR", DEFAULT_BIND_ADDR)?;
        Ok(Self { bind_addr })
    }
}

fn parse_or_default<T>(
    lookup: &impl Fn(&str) -> Option<String>,
    var: &'static str,
    default: &str,
) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = lookup(var).unwrap_or_else(|| default.to_owned());
    value.parse().map_err(|err: T::Err| ConfigError::Invalid {
        var,
        reason: err.to_string(),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|name| vars.get(name).cloned())
    }

    #[test]
    fn uses_defaults_when_nothing_is_set() {
        let config = config_from(&[]).unwrap();
        assert_eq!(config.bind_addr, "0.0.0.0:8080".parse().unwrap());
    }

    #[test]
    fn reads_bind_addr() {
        let config = config_from(&[("BIND_ADDR", "127.0.0.1:9000")]).unwrap();
        assert_eq!(config.bind_addr, "127.0.0.1:9000".parse().unwrap());
    }

    #[test]
    fn rejects_invalid_bind_addr() {
        let err = config_from(&[("BIND_ADDR", "not-an-address")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                var: "BIND_ADDR",
                ..
            }
        ));
    }
}
