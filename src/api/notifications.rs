use axum::{Json, extract::State};
use serde::Serialize;

use super::{AppState, error::ApiError};
use crate::notify::{Notification, SendReport};

#[derive(Debug, Serialize)]
pub struct TestResponse {
    channel: &'static str,
    #[serde(flatten)]
    report: SendReport,
}

/// Sends a test notification to every device, so the user can check that they arrive.
pub async fn test(State(state): State<AppState>) -> Result<Json<TestResponse>, ApiError> {
    let notification = Notification {
        title: "🔔 Notifica di prova".to_owned(),
        body: "Le notifiche di InTime arrivano su questo dispositivo.".to_owned(),
        url: "/notifiche".to_owned(),
        tag: Some("test".to_owned()),
    };
    let report = state.notifier.send(&notification).await?;
    Ok(Json(TestResponse {
        channel: state.notifier.channel(),
        report,
    }))
}
