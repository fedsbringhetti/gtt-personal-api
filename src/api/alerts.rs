use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use chrono::{DateTime, SubsecRound, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use super::{
    AppState,
    error::ApiError,
    stops::{ensure_index_ready, resolve_stop_id, stop_alerts},
};
use crate::{gtt::alerts::Alert, time::TIME_ZONE};

#[derive(Debug, Deserialize)]
pub struct AlertsParams {
    /// A GTT code (`39`) or a full gtfsId.
    stop: Option<String>,
    /// A line name (`4`, `68+`).
    line: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AlertsResponse {
    generated_at: DateTime<Tz>,
    /// When the alerts were read from GTT; `None` before the first read or when GTT's data is
    /// disabled.
    read_at: Option<DateTime<Tz>>,
    alerts: Vec<Alert>,
}

/// GTT's active alerts: those of a stop, of a line, of a line at a stop, or all of them.
pub async fn alerts(
    State(state): State<AppState>,
    params: Result<Query<AlertsParams>, QueryRejection>,
) -> Result<Json<AlertsResponse>, ApiError> {
    let Query(params) = params.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let stop_id = match params.stop.as_deref().map(str::trim) {
        Some(stop) => Some(
            resolve_stop_id(stop)
                .ok_or_else(|| ApiError::InvalidRequest(format!("Invalid stop {stop:?}")))?,
        ),
        None => None,
    };
    let line = params
        .line
        .as_deref()
        .map(str::trim)
        .filter(|line| !line.is_empty());

    let now = state.clock.now();
    let local = |at: DateTime<Utc>| at.with_timezone(&TIME_ZONE).trunc_subsecs(0);
    let read_at = state
        .alerts
        .as_ref()
        .and_then(|alerts| alerts.current().summary())
        .map(|summary| local(summary.read_at));

    let alerts = match (stop_id, line) {
        (Some(stop_id), line) => {
            ensure_index_ready(&state).await?;
            if state.stops.get(&stop_id).await?.is_none() {
                return Err(ApiError::StopNotFound(stop_id));
            }
            let mut alerts = stop_alerts(&state, &stop_id, now).await?;
            if let Some(line) = line {
                alerts.retain(|alert| alert.names_line(line));
            }
            alerts
        }
        (None, Some(line)) => state
            .alerts
            .as_ref()
            .map(|alerts| alerts.current().for_line(line, now))
            .unwrap_or_default(),
        (None, None) => state
            .alerts
            .as_ref()
            .map(|alerts| alerts.current().all(now))
            .unwrap_or_default(),
    };

    Ok(Json(AlertsResponse {
        generated_at: local(now),
        read_at,
        alerts,
    }))
}
