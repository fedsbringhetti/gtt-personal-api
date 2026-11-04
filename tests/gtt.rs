mod common;

use std::time::Duration;

use axum::{Router, http::StatusCode};
use common::{Options, at, fixture, get};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

/// When `fixtures/gtt/` was recorded: 2026-11-04 10:46:30 +01:00. `stop_39.json` and
/// `trip_update.pb` were read from OpenTripPlanner and GTT at the same moment.
const RECORDED: i64 = 1_793_785_590;
const FEED_PATH: &str = "/das_gtfsrt/trip_update.aspx";
const ALL: &str = "/api/stops/39/departures?limit=100&minutes=120";

fn recorded_feed() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/gtt/trip_update.pb"
    ))
    .unwrap()
}

/// Fake OpenTripPlanner serving the recorded departures of stop 39.
async fn otp() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("gtt/stop_39.json")))
        .mount(&server)
        .await;
    server
}

/// Fake GTT answering the trip updates with `response`.
async fn gtt(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(FEED_PATH))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

async fn app(otp: &MockServer, gtt: Option<&MockServer>) -> Router {
    let options = Options {
        gtt_url: gtt.map(|gtt| format!("{}/das_gtfsrt", gtt.uri())),
        ..Options::default()
    };
    common::test_app_with(&otp.uri(), at(RECORDED), Duration::from_secs(2), options)
        .await
        .router
}

fn sources(json: &serde_json::Value) -> Vec<&str> {
    json["departures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|departure| departure["source"].as_str().unwrap())
        .collect()
}

async fn feed_requests(gtt: &MockServer) -> usize {
    gtt.received_requests().await.unwrap().len()
}

#[tokio::test]
async fn urban_departures_use_gtt_predictions() {
    let (otp, gtt) = (
        otp().await,
        gtt(ResponseTemplate::new(200).set_body_bytes(recorded_feed())).await,
    );
    let app = app(&otp, Some(&gtt)).await;

    let (status, json) = get(&app, ALL).await;
    assert_eq!(status, StatusCode::OK);
    let sources = sources(&json);
    assert_eq!(sources.len(), 64);
    assert_eq!(sources.iter().filter(|s| **s == "gtt").count(), 14);

    // The 64 of 10:57:36 is 5 minutes early for GTT (OpenTripPlanner had no real time).
    let early = json["departures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["trip_id"] == "gtt:28485458U")
        .unwrap();
    assert_eq!(early["scheduled"], "2026-11-04T10:57:36+01:00");
    assert_eq!(early["expected"], "2026-11-04T10:52:36+01:00");
    assert_eq!(early["delay_seconds"], -300);
    assert_eq!(early["realtime"], true);
    assert_eq!(early["source"], "gtt");

    let (_, status) = get(&app, "/api/status").await;
    assert_eq!(status["gtt"]["trip_updates"]["ok"], true);
    assert_eq!(
        status["gtt"]["feed"],
        json!({ "runs": 383, "newest_update": "2026-11-04T09:46:29Z" })
    );
    assert_eq!(
        status["gtt"]["comparison"],
        json!({
            "date": "2026-11-04",
            "used": 14,
            "mismatches": 0,
            "compared": 11,
            "median_difference_seconds": 191
        })
    );
}

#[tokio::test]
async fn trip_updates_are_cached() {
    let (otp, gtt) = (
        otp().await,
        gtt(ResponseTemplate::new(200).set_body_bytes(recorded_feed())).await,
    );
    let app = app(&otp, Some(&gtt)).await;

    get(&app, ALL).await;
    let (_, json) = get(&app, ALL).await;
    assert!(sources(&json).contains(&"gtt"));
    assert_eq!(feed_requests(&gtt).await, 1);

    let (_, status) = get(&app, "/api/status").await;
    assert_eq!(
        status["gtt"]["comparison"]["used"], 14,
        "a reload does not count twice"
    );
}

#[tokio::test]
async fn disabled_gtt_leaves_opentripplanner_alone() {
    let otp = otp().await;
    let app = app(&otp, None).await;

    let (status, json) = get(&app, ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sources(&json)
            .iter()
            .all(|s| *s == "otp" || *s == "scheduled")
    );

    let (_, status) = get(&app, "/api/status").await;
    assert_eq!(status["gtt"], serde_json::Value::Null);
}

#[tokio::test]
async fn failing_gtt_falls_back_and_is_left_alone_for_a_while() {
    let (otp, gtt) = (otp().await, gtt(ResponseTemplate::new(503)).await);
    let app = app(&otp, Some(&gtt)).await;

    let (status, json) = get(&app, ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!sources(&json).contains(&"gtt"));
    assert_eq!(feed_requests(&gtt).await, 2, "one retry on 503");

    get(&app, ALL).await;
    assert_eq!(feed_requests(&gtt).await, 2, "not called again right away");

    let (_, status) = get(&app, "/api/status").await;
    assert_eq!(status["gtt"]["trip_updates"]["ok"], false);
    assert_eq!(
        status["gtt"]["trip_updates"]["last_error"],
        "upstream returned HTTP 503"
    );
    assert_eq!(status["gtt"]["feed"], serde_json::Value::Null);
}

#[tokio::test]
async fn invalid_messages_are_rejected_without_retry() {
    let (otp, gtt) = (
        otp().await,
        gtt(ResponseTemplate::new(200).set_body_bytes(b"<html>maintenance</html>".to_vec())).await,
    );
    let app = app(&otp, Some(&gtt)).await;

    let (status, json) = get(&app, ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!sources(&json).contains(&"gtt"));
    assert_eq!(feed_requests(&gtt).await, 1);

    let (_, status) = get(&app, "/api/status").await;
    let error = status["gtt"]["trip_updates"]["last_error"]
        .as_str()
        .unwrap();
    assert!(error.starts_with("invalid upstream response"), "{error}");
}
