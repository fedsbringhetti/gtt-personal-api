mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn health_returns_ok_and_version() {
    let (status, json) = common::get(&common::offline_app().await, "/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}
