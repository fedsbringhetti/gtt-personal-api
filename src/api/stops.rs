use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use chrono::{DateTime, SubsecRound, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use super::{AppState, error::ApiError};
use crate::{
    departures::{Departure, FETCH_WINDOW, Filter, StopRef},
    time::TIME_ZONE,
};

const DEFAULT_MINUTES: i64 = 60;
const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
pub struct DeparturesParams {
    /// Comma-separated line names, e.g. `4,15`.
    lines: Option<String>,
    minutes: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct DeparturesResponse {
    stop: StopRef,
    generated_at: DateTime<Tz>,
    stale: bool,
    departures: Vec<DepartureView>,
}

#[derive(Debug, Serialize)]
struct DepartureView {
    #[serde(flatten)]
    departure: Departure,
    /// Whole minutes until the expected departure.
    minutes: i64,
}

pub async fn departures(
    State(state): State<AppState>,
    Path(stop): Path<String>,
    params: Result<Query<DeparturesParams>, QueryRejection>,
) -> Result<Json<DeparturesResponse>, ApiError> {
    let Query(params) = params.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let stop_id = resolve_stop_id(&stop)
        .ok_or_else(|| ApiError::InvalidRequest(format!("Invalid stop {stop:?}")))?;
    let filter = params.into_filter()?;

    let data = state.departures.get(&stop_id).await?;
    let now = state.clock.now();
    let departures = data
        .select(&filter, now)
        .into_iter()
        .map(|departure| DepartureView {
            minutes: minutes_until(departure.expected, now),
            departure: departure.clone(),
        })
        .collect();

    Ok(Json(DeparturesResponse {
        stop: data.stop.clone(),
        generated_at: now.with_timezone(&TIME_ZONE).trunc_subsecs(0),
        stale: false,
        departures,
    }))
}

impl DeparturesParams {
    fn into_filter(self) -> Result<Filter, ApiError> {
        let max_minutes = (FETCH_WINDOW.as_secs() / 60) as i64;
        let minutes = self.minutes.unwrap_or(DEFAULT_MINUTES);
        if !(1..=max_minutes).contains(&minutes) {
            return Err(ApiError::InvalidRequest(format!(
                "minutes must be between 1 and {max_minutes}"
            )));
        }

        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::InvalidRequest(format!(
                "limit must be between 1 and {MAX_LIMIT}"
            )));
        }

        let lines = self
            .lines
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();

        Ok(Filter {
            lines,
            window: TimeDelta::minutes(minutes),
            limit,
        })
    }
}

/// Accepts a GTT code (`39`) or a full gtfsId (`gtt:39`, `2:S00219`).
fn resolve_stop_id(input: &str) -> Option<String> {
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    match input.split_once(':') {
        Some((feed, code)) if valid(feed) && valid(code) => Some(input.to_owned()),
        Some(_) => None,
        None if valid(input) => Some(format!("gtt:{input}")),
        None => None,
    }
}

fn minutes_until(expected: DateTime<Tz>, now: DateTime<Utc>) -> i64 {
    (expected.with_timezone(&Utc) - now).num_minutes().max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_stop_ids() {
        assert_eq!(resolve_stop_id("39").as_deref(), Some("gtt:39"));
        assert_eq!(resolve_stop_id("gtt:39").as_deref(), Some("gtt:39"));
        assert_eq!(resolve_stop_id("2:S00219").as_deref(), Some("2:S00219"));
        assert_eq!(resolve_stop_id(""), None);
        assert_eq!(resolve_stop_id(":39"), None);
        assert_eq!(resolve_stop_id("gtt:"), None);
        assert_eq!(resolve_stop_id("gtt:3:9"), None);
        assert_eq!(resolve_stop_id("39 OR 1"), None);
    }

    #[test]
    fn minutes_until_rounds_down_and_never_goes_negative() {
        let now = DateTime::from_timestamp(1_000_000, 0).unwrap();
        let at = |secs: i64| {
            DateTime::from_timestamp(1_000_000 + secs, 0)
                .unwrap()
                .with_timezone(&TIME_ZONE)
        };
        assert_eq!(minutes_until(at(179), now), 2);
        assert_eq!(minutes_until(at(30), now), 0);
        assert_eq!(minutes_until(at(-30), now), 0);
    }
}
