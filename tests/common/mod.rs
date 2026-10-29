// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use chrono::{DateTime, Utc};
use gtt_personal_api::{
    api::{self, AppState},
    db,
    departures::DeparturesService,
    favorites::FavoriteStore,
    stops::{Feeds, StopIndex, sync::sync_once},
    time::FixedClock,
    upstream::UpstreamClient,
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

/// First expected departure in `fixtures/stop_39.json` (17:23:53 on 2026-10-29).
pub const STOP_39_FIRST_DEPARTURE: i64 = 1_793_291_033;

/// Fixed "now" for tests that do not depend on departure times.
pub const NOW: i64 = STOP_39_FIRST_DEPARTURE;

pub struct TestApp {
    pub router: Router,
    pub stops: StopIndex,
    pub upstream: UpstreamClient,
}

/// The app with an in-memory database, a fixed clock and default feeds `gtt`.
pub async fn test_app(
    upstream_url: &str,
    now: DateTime<Utc>,
    upstream_timeout: Duration,
) -> TestApp {
    let upstream = UpstreamClient::new(upstream_url.parse().unwrap(), upstream_timeout).unwrap();
    let pool = db::connect_in_memory().await.unwrap();
    let stops = StopIndex::new(pool.clone());
    let router = api::router(AppState {
        departures: Arc::new(DeparturesService::new(
            upstream.clone(),
            Duration::from_secs(20),
        )),
        stops: stops.clone(),
        favorites: FavoriteStore::new(pool),
        default_feeds: Feeds::Only(vec!["gtt".to_owned()]),
        clock: Arc::new(FixedClock(now)),
    });
    TestApp {
        router,
        stops,
        upstream,
    }
}

pub async fn app(upstream_url: &str, now: DateTime<Utc>, upstream_timeout: Duration) -> Router {
    test_app(upstream_url, now, upstream_timeout).await.router
}

/// An app whose upstream is never called.
pub async fn offline_app() -> Router {
    app("http://127.0.0.1:9/", Utc::now(), Duration::from_secs(1)).await
}

/// App whose upstream serves `all_stops.json` (44 real stops around Porta Nuova and a few
/// others), with the index already synced. Keep the server alive for the whole test.
pub async fn synced_app() -> (TestApp, MockServer) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("all_stops.json")))
        .mount(&server)
        .await;
    let app = test_app(&server.uri(), at(NOW), Duration::from_secs(2)).await;
    let count = sync_once(&app.stops, &app.upstream, at(NOW)).await.unwrap();
    assert_eq!(count, 44);
    (app, server)
}

pub fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn at(timestamp: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(timestamp, 0).unwrap()
}

pub async fn get(app: &Router, uri: &str) -> (StatusCode, serde_json::Value) {
    send(app, Method::GET, uri, None).await
}

/// Sends a request with an optional JSON body; the response body is parsed as JSON
/// (`Null` when empty or not JSON).
pub async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    };
    let response = app.clone().oneshot(request.unwrap()).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}
