mod common;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method},
};

use common::{STOP_39_FIRST_DEPARTURE, at, fixture, get};

const TIMEOUT: Duration = Duration::from_secs(2);

/// Fake upstream answering every query with `stop_39.json`.
async fn upstream_with_stop_39() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("stop_39.json")))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn returns_departures_for_a_stop() {
    let server = upstream_with_stop_39().await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE - 30), TIMEOUT);

    let (status, json) = get(&app, "/api/stops/39/departures").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["stop"],
        json!({ "id": "gtt:39", "code": "39", "name": "PORTA NUOVA" })
    );
    assert_eq!(json["stale"], false);
    assert_eq!(json["generated_at"], "2026-10-29T17:23:23+01:00");
    assert_eq!(
        json["departures"][0],
        json!({
            "line": "67",
            "mode": "BUS",
            "headsign": "MONCALIERI, TESTONA - PIAZZA MARCONI",
            "scheduled": "2026-10-29T17:24:00+01:00",
            "expected": "2026-10-29T17:23:53+01:00",
            "delay_seconds": -7,
            "realtime": true,
            "canceled": false,
            "trip_id": "gtt:28459721U",
            "minutes": 0
        })
    );

    let departures = json["departures"].as_array().unwrap();
    assert_eq!(departures.len(), 20, "default limit");
    let expected: Vec<&str> = departures
        .iter()
        .map(|d| d["expected"].as_str().unwrap())
        .collect();
    assert!(expected.is_sorted(), "sorted by expected time");
}

#[tokio::test]
async fn sends_the_full_stop_id_as_a_graphql_variable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({ "variables": { "id": "gtt:39" } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("stop_39.json")))
        .expect(2)
        .mount(&server)
        .await;

    // Two separate apps, so the second request is not served from the cache.
    for uri in ["/api/stops/39/departures", "/api/stops/gtt:39/departures"] {
        let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);
        let (status, _) = get(&app, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn filters_by_lines_window_and_limit() {
    let server = upstream_with_stop_39().await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    let (_, json) = get(&app, "/api/stops/39/departures?lines=4,%2067&limit=100").await;
    let departures = json["departures"].as_array().unwrap();
    assert!(!departures.is_empty());
    assert!(
        departures
            .iter()
            .all(|d| d["line"] == "4" || d["line"] == "67")
    );

    let (_, json) = get(&app, "/api/stops/39/departures?minutes=10&limit=100").await;
    assert!(
        json["departures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["minutes"].as_i64().unwrap() <= 10)
    );

    let (_, json) = get(&app, "/api/stops/39/departures?limit=3").await;
    assert_eq!(json["departures"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn caches_upstream_responses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("stop_39.json")))
        .expect(1)
        .mount(&server)
        .await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    for uri in [
        "/api/stops/39/departures",
        "/api/stops/39/departures?lines=4",
        "/api/stops/gtt:39/departures?limit=5",
    ] {
        let (status, _) = get(&app, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn unknown_stop_returns_404() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": { "stop": null } })))
        .mount(&server)
        .await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    let (status, json) = get(&app, "/api/stops/99999/departures").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "stop_not_found");
    assert_eq!(json["error"]["message"], "Stop gtt:99999 not found");
}

#[tokio::test]
async fn graphql_error_returns_502() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "errors": [{ "message": "Validation error of type FieldUndefined" }]
        })))
        .mount(&server)
        .await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    let (status, json) = get(&app, "/api/stops/39/departures").await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["error"]["code"], "upstream_error");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("FieldUndefined")
    );
}

#[tokio::test]
async fn http_error_without_body_returns_502() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    let (status, json) = get(&app, "/api/stops/39/departures").await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["error"]["message"], "upstream returned HTTP 503");
}

#[tokio::test]
async fn slow_upstream_returns_504() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture("stop_39.json"))
                .set_delay(Duration::from_secs(2)),
        )
        .mount(&server)
        .await;
    let app = common::app(
        &server.uri(),
        at(STOP_39_FIRST_DEPARTURE),
        Duration::from_millis(200),
    );

    let (status, json) = get(&app, "/api/stops/39/departures").await;

    assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(json["error"]["code"], "upstream_timeout");
}

#[tokio::test]
async fn invalid_requests_return_400_without_calling_upstream() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let app = common::app(&server.uri(), at(STOP_39_FIRST_DEPARTURE), TIMEOUT);

    for uri in [
        "/api/stops/39/departures?minutes=abc",
        "/api/stops/39/departures?minutes=0",
        "/api/stops/39/departures?minutes=121",
        "/api/stops/39/departures?limit=0",
        "/api/stops/39/departures?limit=101",
        "/api/stops/a%20b/departures",
        "/api/stops/gtt:3:9/departures",
    ] {
        let (status, json) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(json["error"]["code"], "invalid_request", "{uri}");
    }
}
