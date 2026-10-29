mod common;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{Value, json};
use wiremock::{Mock, ResponseTemplate, matchers::method};

use common::{NOW, at, get, synced_app};

#[tokio::test]
async fn reports_unknown_state_before_any_upstream_call() {
    let app = common::test_app("http://127.0.0.1:9/", at(NOW), Duration::from_secs(1)).await;

    let (status, json) = get(&app.router, "/api/status").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["time"], "2026-10-29T17:23:53+01:00");
    assert_eq!(
        json["upstream"],
        json!({
            "ok": null,
            "last_success": null,
            "last_failure": null,
            "last_error": null,
            "consecutive_failures": 0
        })
    );
    assert_eq!(json["stop_index"], Value::Null);
}

#[tokio::test]
async fn reports_sync_and_upstream_failures() {
    let (app, server) = synced_app().await;

    let (_, json) = get(&app.router, "/api/status").await;
    assert_eq!(json["upstream"]["ok"], true);
    assert!(json["upstream"]["last_success"].is_string());
    assert_eq!(
        json["stop_index"],
        json!({ "synced_at": "2026-10-29T16:23:53Z", "count": 44 })
    );

    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errors": [{ "message": "Validation error of type FieldUndefined" }]
        })))
        .mount(&server)
        .await;
    for _ in 0..2 {
        get(&app.router, "/api/stops/39/departures").await;
    }

    let (_, json) = get(&app.router, "/api/status").await;
    assert_eq!(json["upstream"]["ok"], false);
    assert_eq!(json["upstream"]["consecutive_failures"], 2);
    assert!(
        json["upstream"]["last_error"]
            .as_str()
            .unwrap()
            .contains("FieldUndefined")
    );
    assert!(json["upstream"]["last_success"].is_string(), "kept");
}
