use std::{net::SocketAddr, path::PathBuf, time::Duration};

use chrono::NaiveTime;

use crate::stops::Feeds;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_UPSTREAM_URL: &str = "https://plan.muoversiatorino.it/otp/routers/mato/index/graphql";
const DEFAULT_UPSTREAM_TIMEOUT_SECS: &str = "10";
const DEFAULT_DEPARTURES_CACHE_SECS: &str = "20";
const DEFAULT_DATA_DIR: &str = "data";
const DEFAULT_STOPS_SYNC_TIME: &str = "04:00";
const DEFAULT_FEEDS: &str = "gtt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub upstream_url: reqwest::Url,
    pub upstream_timeout: Duration,
    pub departures_cache_ttl: Duration,
    pub data_dir: PathBuf,
    /// Local time (Europe/Rome) of the nightly stop index sync.
    pub stops_sync_time: NaiveTime,
    /// Feeds searched when a request does not specify any.
    pub default_feeds: Feeds,
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
        Ok(Self {
            bind_addr: parse_or_default(&lookup, "BIND_ADDR", DEFAULT_BIND_ADDR)?,
            upstream_url: parse_or_default(&lookup, "UPSTREAM_URL", DEFAULT_UPSTREAM_URL)?,
            upstream_timeout: Duration::from_secs(parse_or_default(
                &lookup,
                "UPSTREAM_TIMEOUT_SECS",
                DEFAULT_UPSTREAM_TIMEOUT_SECS,
            )?),
            departures_cache_ttl: Duration::from_secs(parse_or_default(
                &lookup,
                "DEPARTURES_CACHE_SECS",
                DEFAULT_DEPARTURES_CACHE_SECS,
            )?),
            data_dir: parse_or_default(&lookup, "DATA_DIR", DEFAULT_DATA_DIR)?,
            stops_sync_time: parse_with(
                &lookup,
                "STOPS_SYNC_TIME",
                DEFAULT_STOPS_SYNC_TIME,
                |value| NaiveTime::parse_from_str(value, "%H:%M").map_err(|err| err.to_string()),
            )?,
            default_feeds: parse_or_default(&lookup, "DEFAULT_FEEDS", DEFAULT_FEEDS)?,
        })
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
    parse_with(lookup, var, default, |value| {
        value.parse().map_err(|err: T::Err| err.to_string())
    })
}

fn parse_with<T>(
    lookup: &impl Fn(&str) -> Option<String>,
    var: &'static str,
    default: &str,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<T, ConfigError> {
    let value = lookup(var).unwrap_or_else(|| default.to_owned());
    parse(&value).map_err(|reason| ConfigError::Invalid { var, value, reason })
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
        assert_eq!(config.upstream_url.as_str(), DEFAULT_UPSTREAM_URL);
        assert_eq!(config.upstream_timeout, Duration::from_secs(10));
        assert_eq!(config.departures_cache_ttl, Duration::from_secs(20));
        assert_eq!(config.data_dir, PathBuf::from("data"));
        assert_eq!(
            config.stops_sync_time,
            NaiveTime::from_hms_opt(4, 0, 0).unwrap()
        );
        assert_eq!(config.default_feeds, Feeds::Only(vec!["gtt".to_owned()]));
    }

    #[test]
    fn reads_stop_index_settings() {
        let config = config_from(&[
            ("DATA_DIR", "/data"),
            ("STOPS_SYNC_TIME", "03:30"),
            ("DEFAULT_FEEDS", "all"),
        ])
        .unwrap();
        assert_eq!(config.data_dir, PathBuf::from("/data"));
        assert_eq!(
            config.stops_sync_time,
            NaiveTime::from_hms_opt(3, 30, 0).unwrap()
        );
        assert_eq!(config.default_feeds, Feeds::All);
    }

    #[test]
    fn rejects_invalid_sync_time_and_feeds() {
        for (var, value) in [("STOPS_SYNC_TIME", "4am"), ("DEFAULT_FEEDS", "gtt;drop")] {
            let err = config_from(&[(var, value)]).unwrap_err();
            assert!(
                matches!(err, ConfigError::Invalid { var: v, .. } if v == var),
                "{var}"
            );
        }
    }

    #[test]
    fn reads_upstream_settings() {
        let config = config_from(&[
            ("UPSTREAM_URL", "http://localhost:9999/graphql"),
            ("UPSTREAM_TIMEOUT_SECS", "3"),
            ("DEPARTURES_CACHE_SECS", "45"),
        ])
        .unwrap();
        assert_eq!(
            config.upstream_url.as_str(),
            "http://localhost:9999/graphql"
        );
        assert_eq!(config.upstream_timeout, Duration::from_secs(3));
        assert_eq!(config.departures_cache_ttl, Duration::from_secs(45));
    }

    #[test]
    fn rejects_invalid_numbers_and_urls() {
        let err = config_from(&[("UPSTREAM_TIMEOUT_SECS", "ten")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                var: "UPSTREAM_TIMEOUT_SECS",
                ..
            }
        ));
        let err = config_from(&[("UPSTREAM_URL", "not a url")]).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::Invalid {
                var: "UPSTREAM_URL",
                ..
            }
        ));
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
