use std::{sync::Arc, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::Tz;
use moka::future::Cache;
use serde::Serialize;

use crate::{
    stops::display_code,
    time::{TIME_ZONE, service_time},
    upstream::{UpstreamClient, UpstreamError, types},
};

/// Departures are fetched for this window and cached; each request filters a part of it.
pub const FETCH_WINDOW: Duration = Duration::from_secs(2 * 60 * 60);
const FETCH_MAX_DEPARTURES: u32 = 200;
const CACHE_CAPACITY: u64 = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct StopRef {
    pub id: String,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Departure {
    pub line: String,
    pub mode: Option<String>,
    pub headsign: Option<String>,
    pub scheduled: DateTime<Tz>,
    pub expected: DateTime<Tz>,
    /// Negative when early.
    pub delay_seconds: i64,
    pub realtime: bool,
    pub canceled: bool,
    pub trip_id: String,
}

#[derive(Debug)]
pub struct StopDepartures {
    pub stop: StopRef,
    /// Sorted by expected time.
    pub departures: Vec<Departure>,
}

#[derive(Debug, Clone)]
pub struct Filter {
    /// Empty means all lines.
    pub lines: Vec<String>,
    pub window: TimeDelta,
    pub limit: usize,
}

impl StopDepartures {
    pub fn from_upstream(stop: types::Stop) -> Self {
        let code = display_code(stop.code, &stop.gtfs_id);
        let mut departures: Vec<_> = stop
            .stoptimes_without_patterns
            .into_iter()
            .filter_map(Departure::from_upstream)
            .collect();
        departures.sort_by_key(|departure| departure.expected);

        Self {
            stop: StopRef {
                id: stop.gtfs_id,
                code,
                name: stop.name,
            },
            departures,
        }
    }

    /// Departures expected from `now` to `now + filter.window`, on the requested lines.
    pub fn select(&self, filter: &Filter, now: DateTime<Utc>) -> Vec<&Departure> {
        let now = now.with_timezone(&TIME_ZONE);
        let until = now + filter.window;
        self.departures
            .iter()
            .filter(|departure| departure.expected >= now && departure.expected <= until)
            .filter(|departure| {
                filter.lines.is_empty()
                    || filter
                        .lines
                        .iter()
                        .any(|line| line.eq_ignore_ascii_case(&departure.line))
            })
            .take(filter.limit)
            .collect()
    }
}

impl Departure {
    fn from_upstream(stoptime: types::Stoptime) -> Option<Self> {
        Some(Self {
            scheduled: service_time(stoptime.service_day, stoptime.scheduled_departure)?,
            expected: service_time(stoptime.service_day, stoptime.realtime_departure)?,
            delay_seconds: stoptime.realtime_departure - stoptime.scheduled_departure,
            realtime: stoptime.realtime,
            canceled: stoptime.realtime_state.as_deref() == Some("CANCELED"),
            line: stoptime.trip.route.short_name.unwrap_or_default(),
            mode: stoptime.trip.route.mode,
            headsign: stoptime.headsign.or(stoptime.trip.trip_headsign),
            trip_id: stoptime.trip.gtfs_id,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeparturesError {
    #[error("stop {0} not found")]
    StopNotFound(String),
    #[error(transparent)]
    Upstream(#[from] UpstreamError),
}

pub struct DeparturesService {
    upstream: UpstreamClient,
    cache: Cache<String, Arc<StopDepartures>>,
}

impl DeparturesService {
    pub fn new(upstream: UpstreamClient, cache_ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(CACHE_CAPACITY)
            .time_to_live(cache_ttl)
            .build();
        Self { upstream, cache }
    }

    /// Departures for a stop, from the cache when fresh. Concurrent requests for the same stop
    /// share a single upstream call; errors are not cached.
    pub async fn get(&self, stop_id: &str) -> Result<Arc<StopDepartures>, Arc<DeparturesError>> {
        self.cache
            .try_get_with(stop_id.to_owned(), async {
                let stop = self
                    .upstream
                    .stop_departures(stop_id, FETCH_WINDOW, FETCH_MAX_DEPARTURES)
                    .await?
                    .ok_or_else(|| DeparturesError::StopNotFound(stop_id.to_owned()))?;
                Ok::<_, DeparturesError>(Arc::new(StopDepartures::from_upstream(stop)))
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SERVICE_DAY: i64 = 1_793_228_400; // 2026-10-29

    fn stoptime(line: &str, scheduled: i64, expected: i64, state: &str) -> serde_json::Value {
        json!({
            "scheduledDeparture": scheduled,
            "realtimeDeparture": expected,
            "realtime": state == "UPDATED",
            "realtimeState": state,
            "serviceDay": SERVICE_DAY,
            "headsign": null,
            "trip": {
                "gtfsId": format!("gtt:{line}-{scheduled}"),
                "tripHeadsign": "CAPOLINEA",
                "route": { "shortName": line, "mode": "BUS" }
            }
        })
    }

    fn stop(code: Option<&str>, stoptimes: Vec<serde_json::Value>) -> StopDepartures {
        let stop: types::Stop = serde_json::from_value(json!({
            "gtfsId": "gtt:8224",
            "code": code,
            "name": "METRO PORTA NUOVA",
            "stoptimesWithoutPatterns": stoptimes,
        }))
        .unwrap();
        StopDepartures::from_upstream(stop)
    }

    fn at(hours: i64, minutes: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(SERVICE_DAY + hours * 3600 + minutes * 60, 0).unwrap()
    }

    fn filter(lines: &[&str], window_minutes: i64, limit: usize) -> Filter {
        Filter {
            lines: lines.iter().map(|line| line.to_string()).collect(),
            window: TimeDelta::minutes(window_minutes),
            limit,
        }
    }

    #[test]
    fn falls_back_to_code_from_id() {
        assert_eq!(stop(None, vec![]).stop.code, "8224");
        assert_eq!(stop(Some(""), vec![]).stop.code, "8224");
        assert_eq!(stop(Some("39"), vec![]).stop.code, "39");
    }

    #[test]
    fn maps_departure_fields() {
        let data = stop(None, vec![stoptime("9B", 28_800, 28_980, "UPDATED")]);
        let departure = &data.departures[0];
        assert_eq!(departure.line, "9B");
        assert_eq!(departure.headsign.as_deref(), Some("CAPOLINEA"));
        assert_eq!(
            departure.scheduled.to_rfc3339(),
            "2026-10-29T08:00:00+01:00"
        );
        assert_eq!(departure.expected.to_rfc3339(), "2026-10-29T08:03:00+01:00");
        assert_eq!(departure.delay_seconds, 180);
        assert!(departure.realtime);
        assert!(!departure.canceled);
    }

    #[test]
    fn flags_canceled_departures() {
        let data = stop(None, vec![stoptime("4", 28_800, 28_800, "CANCELED")]);
        assert!(data.departures[0].canceled);
    }

    #[test]
    fn sorts_by_expected_time() {
        let data = stop(
            None,
            vec![
                stoptime("4", 28_800, 29_400, "UPDATED"),
                stoptime("15", 29_100, 29_100, "SCHEDULED"),
            ],
        );
        let lines: Vec<_> = data.departures.iter().map(|d| d.line.as_str()).collect();
        assert_eq!(lines, ["15", "4"]);
    }

    #[test]
    fn select_applies_window_lines_and_limit() {
        let data = stop(
            None,
            vec![
                stoptime("4", 28_500, 28_500, "SCHEDULED"), // 07:55, already gone
                stoptime("4", 28_800, 28_800, "SCHEDULED"), // 08:00
                stoptime("15", 29_400, 29_400, "SCHEDULED"), // 08:10
                stoptime("4", 30_600, 30_600, "SCHEDULED"), // 08:30
                stoptime("4", 32_400, 32_400, "SCHEDULED"), // 09:00, outside 45 minutes
            ],
        );
        let lines_at = |filter: &Filter| -> Vec<String> {
            data.select(filter, at(8, 0))
                .iter()
                .map(|d| format!("{} {}", d.line, d.expected.format("%H:%M")))
                .collect()
        };

        assert_eq!(
            lines_at(&filter(&[], 45, 10)),
            ["4 08:00", "15 08:10", "4 08:30"]
        );
        assert_eq!(lines_at(&filter(&["4"], 45, 10)), ["4 08:00", "4 08:30"]);
        assert_eq!(lines_at(&filter(&[], 45, 1)), ["4 08:00"]);
    }

    #[test]
    fn line_filter_ignores_case() {
        let data = stop(None, vec![stoptime("9B", 28_800, 28_800, "SCHEDULED")]);
        assert_eq!(data.select(&filter(&["9b"], 60, 10), at(8, 0)).len(), 1);
    }
}
