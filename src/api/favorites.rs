use axum::{
    Json,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
};
use serde::{Deserialize, Deserializer, Serialize};

use super::{AppState, error::ApiError, stops::resolve_stop_id};
use crate::{
    favorites::{Favorite, FavoriteUpdate, NewFavorite},
    stops::StopInfo,
};

const MAX_LABEL_LEN: usize = 100;
const MAX_LINES: usize = 20;
const MAX_LINE_LEN: usize = 10;

#[derive(Debug, Serialize)]
pub struct FavoritesResponse {
    favorites: Vec<FavoriteView>,
}

#[derive(Debug, Serialize)]
pub struct FavoriteView {
    #[serde(flatten)]
    favorite: Favorite,
    /// `None` when the stop is not in the index.
    stop: Option<StopInfo>,
    /// The stop no longer exists after a sync.
    missing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateFavorite {
    /// GTT code (`39`) or full gtfsId (`gtt:39`).
    stop: String,
    label: Option<String>,
    #[serde(default)]
    lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateFavorite {
    /// Missing: unchanged; `null`: cleared.
    #[serde(default, deserialize_with = "present")]
    label: Option<Option<String>>,
    lines: Option<Vec<String>>,
    position: Option<usize>,
}

/// Tells a field set to `null` (`Some(None)`) apart from a missing one (`None`).
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<FavoritesResponse>, ApiError> {
    let index_ready = state.stops.last_sync().await?.is_some();
    let mut favorites = Vec::new();
    for favorite in state.favorites.list().await? {
        favorites.push(view(&state, favorite, index_ready).await?);
    }
    Ok(Json(FavoritesResponse { favorites }))
}

pub async fn create(
    State(state): State<AppState>,
    body: Result<Json<CreateFavorite>, JsonRejection>,
) -> Result<(StatusCode, Json<FavoriteView>), ApiError> {
    let Json(body) = body.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let stop_id = resolve_stop_id(&body.stop)
        .ok_or_else(|| ApiError::InvalidRequest(format!("Invalid stop {:?}", body.stop)))?;
    let label = clean_label(body.label)?;
    let lines = clean_lines(body.lines)?;

    // The index must know the stop, otherwise a typo would create a dead favorite.
    if state.stops.last_sync().await?.is_none() {
        return Err(ApiError::IndexNotReady);
    }
    let stop = state
        .stops
        .get(&stop_id)
        .await?
        .ok_or_else(|| ApiError::StopNotFound(stop_id.clone()))?;

    let favorite = state
        .favorites
        .create(
            NewFavorite {
                stop_id,
                label,
                lines,
            },
            state.clock.now(),
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(FavoriteView {
            favorite,
            stop: Some(stop),
            missing: false,
        }),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<UpdateFavorite>, JsonRejection>,
) -> Result<Json<FavoriteView>, ApiError> {
    let Path(id) = id.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let Json(body) = body.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let update = FavoriteUpdate {
        label: body.label.map(clean_label).transpose()?,
        lines: body.lines.map(clean_lines).transpose()?,
        position: body.position,
    };

    let favorite = state
        .favorites
        .update(id, update)
        .await?
        .ok_or(ApiError::FavoriteNotFound(id))?;
    let index_ready = state.stops.last_sync().await?.is_some();
    Ok(Json(view(&state, favorite, index_ready).await?))
}

pub async fn delete(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let Path(id) = id.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    if state.favorites.delete(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::FavoriteNotFound(id))
    }
}

async fn view(
    state: &AppState,
    favorite: Favorite,
    index_ready: bool,
) -> Result<FavoriteView, ApiError> {
    let stop = state.stops.get(&favorite.stop_id).await?;
    Ok(FavoriteView {
        // Before the first sync every stop would look missing.
        missing: index_ready && stop.is_none(),
        stop,
        favorite,
    })
}

/// Trimmed; empty becomes `None`.
fn clean_label(label: Option<String>) -> Result<Option<String>, ApiError> {
    let Some(label) = label.map(|label| label.trim().to_owned()) else {
        return Ok(None);
    };
    if label.chars().count() > MAX_LABEL_LEN {
        return Err(ApiError::InvalidRequest(format!(
            "label must be at most {MAX_LABEL_LEN} characters"
        )));
    }
    Ok((!label.is_empty()).then_some(label))
}

/// Trimmed, without empty entries or duplicates, in the given order.
fn clean_lines(lines: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut cleaned: Vec<String> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || cleaned.iter().any(|other| other.eq_ignore_ascii_case(line)) {
            continue;
        }
        if line.chars().count() > MAX_LINE_LEN {
            return Err(ApiError::InvalidRequest(format!(
                "line names must be at most {MAX_LINE_LEN} characters"
            )));
        }
        cleaned.push(line.to_owned());
    }
    if cleaned.len() > MAX_LINES {
        return Err(ApiError::InvalidRequest(format!(
            "at most {MAX_LINES} lines"
        )));
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_labels() {
        assert_eq!(clean_label(None).unwrap(), None);
        assert_eq!(clean_label(Some("  ".into())).unwrap(), None);
        assert_eq!(
            clean_label(Some(" Casa → lavoro ".into())).unwrap(),
            Some("Casa → lavoro".to_owned())
        );
        assert!(clean_label(Some("x".repeat(101))).is_err());
    }

    #[test]
    fn cleans_lines() {
        let lines = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            clean_lines(lines(&[" 4", "15", "", "4 ", "9b", "9B"])).unwrap(),
            ["4", "15", "9b"]
        );
        assert!(clean_lines(lines(&["12345678901"])).is_err());
        let many: Vec<String> = (0..21).map(|n| n.to_string()).collect();
        assert!(clean_lines(many).is_err());
    }

    #[test]
    fn update_body_tells_null_from_missing() {
        let parse = |json: &str| serde_json::from_str::<UpdateFavorite>(json).unwrap().label;
        assert_eq!(parse("{}"), None);
        assert_eq!(parse(r#"{"label": null}"#), Some(None));
        assert_eq!(parse(r#"{"label": "x"}"#), Some(Some("x".to_owned())));
    }
}
