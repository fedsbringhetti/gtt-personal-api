// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, Utc};
use gtt_personal_api::{
    api::{self, AppState},
    departures::DeparturesService,
    time::FixedClock,
    upstream::UpstreamClient,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// First expected departure in `fixtures/stop_39.json` (17:23:53 on 2026-10-29).
pub const STOP_39_FIRST_DEPARTURE: i64 = 1_793_291_033;

pub fn app(upstream_url: &str, now: DateTime<Utc>, upstream_timeout: Duration) -> Router {
    let upstream = UpstreamClient::new(upstream_url.parse().unwrap(), upstream_timeout).unwrap();
    api::router(AppState {
        departures: Arc::new(DeparturesService::new(upstream, Duration::from_secs(20))),
        clock: Arc::new(FixedClock(now)),
    })
}

/// An app whose upstream is never called.
pub fn offline_app() -> Router {
    app("http://127.0.0.1:9/", Utc::now(), Duration::from_secs(1))
}

pub fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn at(timestamp: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(timestamp, 0).unwrap()
}

pub async fn get(app: &Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}
