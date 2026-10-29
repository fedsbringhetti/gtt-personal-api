use std::sync::Arc;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::{
    departures::DeparturesError, favorites::FavoritesError, stops::IndexError,
    upstream::UpstreamError,
};

/// Errors returned to API clients as `{"error": {"code": ..., "message": ...}}`.
#[derive(Debug)]
pub enum ApiError {
    InvalidRequest(String),
    StopNotFound(String),
    FavoriteNotFound(i64),
    UpstreamTimeout,
    Upstream(String),
    /// The stop index has not been synced yet.
    IndexNotReady,
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::InvalidRequest(message) => (StatusCode::BAD_REQUEST, "invalid_request", message),
            Self::StopNotFound(stop) => (
                StatusCode::NOT_FOUND,
                "stop_not_found",
                format!("Stop {stop} not found"),
            ),
            Self::FavoriteNotFound(id) => (
                StatusCode::NOT_FOUND,
                "favorite_not_found",
                format!("Favorite {id} not found"),
            ),
            Self::UpstreamTimeout => (
                StatusCode::GATEWAY_TIMEOUT,
                "upstream_timeout",
                "The upstream service did not answer in time".to_owned(),
            ),
            Self::Upstream(message) => (StatusCode::BAD_GATEWAY, "upstream_error", message),
            Self::IndexNotReady => (
                StatusCode::SERVICE_UNAVAILABLE,
                "index_not_ready",
                "The stop index is still being downloaded, try again shortly".to_owned(),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Internal error".to_owned(),
            ),
        };
        let body = json!({ "error": { "code": code, "message": message } });
        (status, Json(body)).into_response()
    }
}

impl From<Arc<DeparturesError>> for ApiError {
    fn from(err: Arc<DeparturesError>) -> Self {
        match &*err {
            DeparturesError::StopNotFound(stop) => Self::StopNotFound(stop.clone()),
            DeparturesError::Upstream(err) => {
                tracing::warn!("{err}");
                match err {
                    UpstreamError::Timeout => Self::UpstreamTimeout,
                    err => Self::Upstream(err.to_string()),
                }
            }
        }
    }
}

impl From<IndexError> for ApiError {
    fn from(err: IndexError) -> Self {
        tracing::error!("stop index: {err}");
        Self::Internal
    }
}

impl From<FavoritesError> for ApiError {
    fn from(err: FavoritesError) -> Self {
        tracing::error!("favorites: {err}");
        Self::Internal
    }
}
