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
    stops::{Feeds, NearbyStop, StopInfo},
    time::TIME_ZONE,
};

const DEFAULT_MINUTES: i64 = 60;
const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;
const MAX_QUERY_LEN: usize = 100;
const DEFAULT_RADIUS_M: f64 = 300.0;
const MAX_RADIUS_M: f64 = 2_000.0;

#[derive(Debug, Serialize)]
pub struct StopsResponse<T> {
    stops: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    q: Option<String>,
    /// `all` or a comma-separated list; defaults to `DEFAULT_FEEDS`.
    feeds: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct NearbyParams {
    lat: Option<f64>,
    lon: Option<f64>,
    radius: Option<f64>,
    feeds: Option<String>,
    limit: Option<usize>,
}

pub async fn search(
    State(state): State<AppState>,
    params: Result<Query<SearchParams>, QueryRejection>,
) -> Result<Json<StopsResponse<StopInfo>>, ApiError> {
    let Query(params) = params.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let query = params.q.as_deref().map(str::trim).unwrap_or_default();
    if query.is_empty() || query.chars().count() > MAX_QUERY_LEN {
        return Err(ApiError::InvalidRequest(format!(
            "q must be between 1 and {MAX_QUERY_LEN} characters"
        )));
    }
    let feeds = feeds_or_default(params.feeds, &state.default_feeds)?;
    let limit = check_range("limit", params.limit.unwrap_or(DEFAULT_LIMIT), 1, MAX_LIMIT)?;

    ensure_index_ready(&state).await?;
    let stops = state.stops.search(query, &feeds, limit as u32).await?;
    Ok(Json(StopsResponse { stops }))
}

pub async fn nearby(
    State(state): State<AppState>,
    params: Result<Query<NearbyParams>, QueryRejection>,
) -> Result<Json<StopsResponse<NearbyStop>>, ApiError> {
    let Query(params) = params.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let lat = params
        .lat
        .ok_or_else(|| ApiError::InvalidRequest("lat is required".to_owned()))?;
    let lon = params
        .lon
        .ok_or_else(|| ApiError::InvalidRequest("lon is required".to_owned()))?;
    let lat = check_range("lat", lat, -90.0, 90.0)?;
    let lon = check_range("lon", lon, -180.0, 180.0)?;
    let radius = check_range(
        "radius",
        params.radius.unwrap_or(DEFAULT_RADIUS_M),
        1.0,
        MAX_RADIUS_M,
    )?;
    let feeds = feeds_or_default(params.feeds, &state.default_feeds)?;
    let limit = check_range("limit", params.limit.unwrap_or(DEFAULT_LIMIT), 1, MAX_LIMIT)?;

    ensure_index_ready(&state).await?;
    let stops = state.stops.nearby(lat, lon, radius, &feeds, limit).await?;
    Ok(Json(StopsResponse { stops }))
}

pub async fn details(
    State(state): State<AppState>,
    Path(stop): Path<String>,
) -> Result<Json<StopInfo>, ApiError> {
    let stop_id = resolve_stop_id(&stop)
        .ok_or_else(|| ApiError::InvalidRequest(format!("Invalid stop {stop:?}")))?;
    ensure_index_ready(&state).await?;
    state
        .stops
        .get(&stop_id)
        .await?
        .map(Json)
        .ok_or(ApiError::StopNotFound(stop_id))
}

/// Before the first sync the index is empty, and "no results" would be misleading.
async fn ensure_index_ready(state: &AppState) -> Result<(), ApiError> {
    match state.stops.last_sync().await? {
        Some(_) => Ok(()),
        None => Err(ApiError::IndexNotReady),
    }
}

fn feeds_or_default(feeds: Option<String>, default: &Feeds) -> Result<Feeds, ApiError> {
    match feeds {
        Some(feeds) => feeds
            .parse()
            .map_err(|err| ApiError::InvalidRequest(format!("feeds: {err}"))),
        None => Ok(default.clone()),
    }
}

/// `value` if it is within `min..=max` (NaN never is).
fn check_range<T: PartialOrd + std::fmt::Display>(
    name: &str,
    value: T,
    min: T,
    max: T,
) -> Result<T, ApiError> {
    if value >= min && value <= max {
        Ok(value)
    } else {
        Err(ApiError::InvalidRequest(format!(
            "{name} must be between {min} and {max}"
        )))
    }
}

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
    /// When the data was received from the upstream.
    fetched_at: DateTime<Tz>,
    /// The upstream is failing and this is the last good data (at most 2 minutes old).
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

    let fetched = state.departures.get(&stop_id).await?;
    let data = &fetched.data;
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
        fetched_at: data.fetched_at.with_timezone(&TIME_ZONE).trunc_subsecs(0),
        stale: fetched.stale,
        departures,
    }))
}

impl DeparturesParams {
    fn into_filter(self) -> Result<Filter, ApiError> {
        let max_minutes = (FETCH_WINDOW.as_secs() / 60) as i64;
        let minutes = check_range(
            "minutes",
            self.minutes.unwrap_or(DEFAULT_MINUTES),
            1,
            max_minutes,
        )?;
        let limit = check_range("limit", self.limit.unwrap_or(DEFAULT_LIMIT), 1, MAX_LIMIT)?;

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
pub(super) fn resolve_stop_id(input: &str) -> Option<String> {
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

    #[test]
    fn check_range_rejects_out_of_range_and_nan() {
        assert!(check_range("x", 5, 1, 10).is_ok());
        assert!(check_range("x", 0, 1, 10).is_err());
        assert!(check_range("x", 11, 1, 10).is_err());
        assert!(check_range("x", f64::NAN, -90.0, 90.0).is_err());
    }
}
