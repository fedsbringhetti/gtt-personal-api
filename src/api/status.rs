use axum::{Json, extract::State};
use chrono::{DateTime, SubsecRound, Utc};
use chrono_tz::Tz;
use serde::Serialize;

use super::{AppState, error::ApiError};
use crate::{
    gtt::{FeedSummary, comparison::ComparisonSummary},
    time::TIME_ZONE,
    upstream::health::HealthSnapshot,
};

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    version: &'static str,
    time: DateTime<Tz>,
    /// Outcome of the most recent upstream calls (departures and stop syncs).
    upstream: HealthSnapshot,
    /// `None` until the first successful sync.
    stop_index: Option<StopIndexStatus>,
    /// GTT's real-time data; `None` when disabled (`GTT_REALTIME_URL` empty).
    gtt: Option<GttStatus>,
}

#[derive(Debug, Serialize)]
struct GttStatus {
    /// Outcome of the most recent reads of the trip updates.
    trip_updates: HealthSnapshot,
    /// The last trip updates read; `None` until the first success.
    feed: Option<FeedSummary>,
    /// Today's comparison with OpenTripPlanner.
    comparison: ComparisonSummary,
}

#[derive(Debug, Serialize)]
struct StopIndexStatus {
    synced_at: DateTime<Utc>,
    count: i64,
}

/// Diagnostic overview. Unlike `/health`, it reports on dependencies, not just the process.
pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponse>, ApiError> {
    let stop_index = state.stops.last_sync().await?.map(|sync| StopIndexStatus {
        synced_at: sync.at,
        count: sync.count,
    });
    let now = state.clock.now().with_timezone(&TIME_ZONE);
    let gtt = state.gtt.as_ref().map(|gtt| GttStatus {
        trip_updates: gtt.health().snapshot(),
        feed: gtt.latest(),
        comparison: gtt.comparisons.summary(now.date_naive()),
    });
    Ok(Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        time: now.trunc_subsecs(0),
        upstream: state.upstream_health.snapshot(),
        stop_index,
        gtt,
    }))
}
