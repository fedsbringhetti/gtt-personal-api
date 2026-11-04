//! GTT's alerts: reading and linking to stops, as `/api/status` reports them.

mod common;

use std::{
    io::{Cursor, Write},
    time::Duration,
};

use axum::http::StatusCode;
use common::{Options, TestApp, at, fixture, get};
use gtt_personal_api::stops::sync::sync_once;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_string_contains, method, path},
};

/// When `fixtures/gtt/stop_39.json` was recorded (2026-11-04 10:46:30 +01:00). The alerts in
/// `fixtures/gtt/alerts.pb` were recorded the same day at 16:03:58; all 119 were already active.
const NOW: i64 = 1_793_785_590;
const ALERTS_PATH: &str = "/das_gtfsrt/alerts.aspx";
const ARCHIVE_PATH: &str = "/open_data/gtt_gtfs.zip";

fn file(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/gtt/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// GTT's archive with the real rows of `fixtures/gtt/stops.txt`: 26 of the stops around Porta
/// Nuova in `fixtures/all_stops.json`.
fn archive() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("stops.txt", options).unwrap();
    zip.write_all(&file("stops.txt")).unwrap();
    zip.finish().unwrap().into_inner()
}

/// Fake OpenTripPlanner (all stops, departures of stop 39) and fake GTT (alerts, archive).
async fn servers() -> (MockServer, MockServer) {
    let otp = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("AllStops"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("all_stops.json")))
        .mount(&otp)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("StopDepartures"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("gtt/stop_39.json")))
        .mount(&otp)
        .await;
    let gtt = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ALERTS_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(file("alerts.pb")))
        .mount(&gtt)
        .await;
    Mock::given(method("GET"))
        .and(path(ARCHIVE_PATH))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Last-Modified", "Sun, 01 Nov 2026 02:26:59 GMT")
                .set_body_bytes(archive()),
        )
        .mount(&gtt)
        .await;
    (otp, gtt)
}

/// The app with GTT's data on, the stop index synced and, when `with_static`, the static data
/// downloaded; the alerts are read once.
async fn app(otp: &MockServer, gtt: &MockServer, with_static: bool) -> TestApp {
    let options = Options {
        gtt_url: Some(format!("{}/das_gtfsrt", gtt.uri())),
        gtt_gtfs_url: with_static.then(|| format!("{}{ARCHIVE_PATH}", gtt.uri())),
        ..Options::default()
    };
    let app = common::test_app_with(&otp.uri(), at(NOW), Duration::from_secs(2), options).await;
    sync_once(&app.stops, &app.upstream, at(NOW)).await.unwrap();
    if let Some(gtt_static) = &app.gtt_static {
        gtt_static.check(at(NOW)).await.unwrap();
    }
    let alerts = app.alerts.as_ref().unwrap();
    assert_eq!(alerts.refresh(at(NOW)).await.unwrap(), 119);
    app
}

#[tokio::test]
async fn status_reports_alerts_and_static_data() {
    let (otp, gtt) = servers().await;
    let app = app(&otp, &gtt, true).await;

    let (status, json) = get(&app.router, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    let alerts = &json["gtt"]["alerts"];
    assert_eq!(alerts["feed"]["ok"], true);
    assert_eq!(alerts["read"]["alerts"], 119);
    // The fixture's static data knows 26 stops: most stops named by alerts are unknown.
    assert_eq!(alerts["read"]["unknown_stops"], 3_971);
    let static_data = &json["gtt"]["static_data"];
    assert_eq!(static_data["download"]["ok"], true);
    assert_eq!(static_data["data"]["stops"], 26);
    assert_eq!(static_data["data"]["version"], "2026-11-01T02:26:59Z");
}
