use axum::{Json, extract::State};
use chrono::{DateTime, SubsecRound, Utc};
use chrono_tz::Tz;
use serde::Serialize;

use super::{AppState, error::ApiError};
use crate::{time::TIME_ZONE, upstream::health::HealthSnapshot};

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    version: &'static str,
    time: DateTime<Tz>,
    /// Outcome of the most recent upstream calls (departures and stop syncs).
    upstream: HealthSnapshot,
    /// `None` until the first successful sync.
    stop_index: Option<StopIndexStatus>,
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
    Ok(Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        time: state.clock.now().with_timezone(&TIME_ZONE).trunc_subsecs(0),
        upstream: state.upstream_health.snapshot(),
        stop_index,
    }))
}
