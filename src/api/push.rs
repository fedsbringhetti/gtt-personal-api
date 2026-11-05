use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{AppState, error::ApiError};
use crate::push::{NewSubscription, Subscription, decode_keys};

#[derive(Debug, Serialize)]
pub struct PublicKeyResponse {
    public_key: String,
}

/// The browser's `PushSubscription.toJSON()`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeRequest {
    endpoint: String,
    /// Browsers send it (usually `null`); we do not use it.
    #[serde(default, rename = "expirationTime")]
    _expiration_time: Option<serde_json::Value>,
    keys: Keys,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keys {
    p256dh: String,
    auth: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsubscribeRequest {
    endpoint: String,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionView {
    id: i64,
    label: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<Subscription> for SubscriptionView {
    fn from(subscription: Subscription) -> Self {
        Self {
            id: subscription.id,
            label: subscription.label,
            created_at: subscription.created_at,
        }
    }
}

pub async fn public_key(
    State(state): State<AppState>,
) -> Result<Json<PublicKeyResponse>, ApiError> {
    let key = state.vapid_public_key.ok_or(ApiError::PushDisabled)?;
    Ok(Json(PublicKeyResponse {
        public_key: key.to_string(),
    }))
}

pub async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<SubscribeRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SubscriptionView>), ApiError> {
    if state.vapid_public_key.is_none() {
        return Err(ApiError::PushDisabled);
    }
    let Json(body) = body.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    let endpoint = state
        .push_endpoints
        .check(&body.endpoint)
        .map_err(|reason| ApiError::InvalidRequest(format!("Invalid endpoint: {reason}")))?;
    decode_keys(&body.keys.p256dh, &body.keys.auth)
        .map_err(|reason| ApiError::InvalidRequest(format!("Invalid keys: {reason}")))?;

    let label = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .and_then(device_label);
    let (subscription, created) = state
        .subscriptions
        .upsert(
            NewSubscription {
                endpoint: endpoint.as_str().to_owned(),
                p256dh: body.keys.p256dh,
                auth: body.keys.auth,
                label,
            },
            state.clock.now(),
        )
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(subscription.into())))
}

/// Succeeds also when the endpoint is unknown: the browser may have lost track of it.
pub async fn unsubscribe(
    State(state): State<AppState>,
    body: Result<Json<UnsubscribeRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(body) = body.map_err(|err| ApiError::InvalidRequest(err.body_text()))?;
    state
        .subscriptions
        .delete_by_endpoint(&body.endpoint)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A short device name from the `User-Agent` (decision AR). iPhone and iPad come before
/// `Macintosh` because their user agents also say `like Mac OS X` and iPadOS can report itself
/// as a Mac.
fn device_label(user_agent: &str) -> Option<String> {
    const DEVICES: [(&str, &str); 5] = [
        ("iPhone", "iPhone"),
        ("iPad", "iPad"),
        ("Android", "Android"),
        ("Macintosh", "Mac"),
        ("Windows", "Windows"),
    ];
    DEVICES
        .iter()
        .find(|(marker, _)| user_agent.contains(marker))
        .map(|(_, label)| (*label).to_owned())
}

#[cfg(test)]
mod tests {
    use super::device_label;

    #[test]
    fn labels_come_from_the_user_agent() {
        let label = |ua| device_label(ua);
        assert_eq!(
            label("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)").as_deref(),
            Some("iPhone")
        );
        assert_eq!(
            label("Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X)").as_deref(),
            Some("iPad")
        );
        assert_eq!(
            label("Mozilla/5.0 (Linux; Android 14; Pixel 8)").as_deref(),
            Some("Android")
        );
        assert_eq!(
            label("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)").as_deref(),
            Some("Mac")
        );
        assert_eq!(
            label("Mozilla/5.0 (Windows NT 10.0; Win64; x64)").as_deref(),
            Some("Windows")
        );
        assert_eq!(label("curl/8.0"), None);
        assert_eq!(label(""), None);
    }
}
