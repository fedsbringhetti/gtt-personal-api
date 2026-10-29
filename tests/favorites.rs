mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use gtt_personal_api::stops::sync::sync_once;
use serde_json::{Value, json};
use wiremock::{Mock, ResponseTemplate, matchers::method};

use common::{NOW, at, fixture, get, send, synced_app};

async fn create(app: &axum::Router, body: Value) -> (StatusCode, Value) {
    send(app, Method::POST, "/api/favorites", Some(body)).await
}

async fn list(app: &axum::Router) -> Vec<Value> {
    let (status, json) = get(app, "/api/favorites").await;
    assert_eq!(status, StatusCode::OK);
    json["favorites"].as_array().unwrap().clone()
}

/// `(id, stop_id, position)` of every favorite, in list order.
async fn order(app: &axum::Router) -> Vec<(i64, String, i64)> {
    list(app)
        .await
        .iter()
        .map(|f| {
            (
                f["id"].as_i64().unwrap(),
                f["stop_id"].as_str().unwrap().to_owned(),
                f["position"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn creates_a_favorite_with_stop_details() {
    let (app, _server) = synced_app().await;

    let (status, json) = create(
        &app.router,
        json!({ "stop": "39", "label": " Casa → lavoro ", "lines": [" 9B", "52", "9b", ""] }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["stop_id"], "gtt:39");
    assert_eq!(json["label"], "Casa → lavoro");
    assert_eq!(json["lines"], json!(["9B", "52"]));
    assert_eq!(json["position"], 0);
    assert_eq!(json["created_at"], "2026-10-29T16:23:53Z");
    assert_eq!(json["stop"]["name"], "PORTA NUOVA");
    assert_eq!(json["missing"], false);

    let favorites = list(&app.router).await;
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0]["id"], json["id"]);
    assert_eq!(favorites[0]["stop"]["code"], "39");
}

#[tokio::test]
async fn new_favorites_go_to_the_end() {
    let (app, _server) = synced_app().await;

    for stop in ["39", "gtt:40", "8224"] {
        let (status, _) = create(&app.router, json!({ "stop": stop })).await;
        assert_eq!(status, StatusCode::CREATED, "{stop}");
    }

    let stops: Vec<(String, i64)> = order(&app.router)
        .await
        .into_iter()
        .map(|(_, stop, position)| (stop, position))
        .collect();
    assert_eq!(
        stops,
        [
            ("gtt:39".to_owned(), 0),
            ("gtt:40".to_owned(), 1),
            ("gtt:8224".to_owned(), 2)
        ]
    );
}

#[tokio::test]
async fn updates_label_lines_and_position() {
    let (app, _server) = synced_app().await;
    let mut ids = Vec::new();
    for stop in ["39", "40", "253"] {
        let (_, json) = create(&app.router, json!({ "stop": stop, "label": "x" })).await;
        ids.push(json["id"].as_i64().unwrap());
    }
    let uri = format!("/api/favorites/{}", ids[2]);

    let (status, json) = send(
        &app.router,
        Method::PATCH,
        &uri,
        Some(json!({ "label": "Stazione", "lines": ["4"], "position": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["label"], "Stazione");
    assert_eq!(json["lines"], json!(["4"]));
    assert_eq!(json["position"], 0);
    assert_eq!(
        order(&app.router).await,
        [
            (ids[2], "gtt:253".to_owned(), 0),
            (ids[0], "gtt:39".to_owned(), 1),
            (ids[1], "gtt:40".to_owned(), 2)
        ]
    );

    // null clears the label; missing fields stay as they are.
    let (_, json) = send(
        &app.router,
        Method::PATCH,
        &uri,
        Some(json!({ "label": null })),
    )
    .await;
    assert_eq!(json["label"], Value::Null);
    assert_eq!(json["lines"], json!(["4"]));

    // Positions past the end move the favorite to the end.
    let (_, json) = send(
        &app.router,
        Method::PATCH,
        &uri,
        Some(json!({ "position": 99 })),
    )
    .await;
    assert_eq!(json["position"], 2);
}

#[tokio::test]
async fn delete_keeps_positions_contiguous() {
    let (app, _server) = synced_app().await;
    let mut ids = Vec::new();
    for stop in ["39", "40", "253"] {
        let (_, json) = create(&app.router, json!({ "stop": stop })).await;
        ids.push(json["id"].as_i64().unwrap());
    }

    let uri = format!("/api/favorites/{}", ids[0]);
    let (status, body) = send(&app.router, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);

    assert_eq!(
        order(&app.router).await,
        [
            (ids[1], "gtt:40".to_owned(), 0),
            (ids[2], "gtt:253".to_owned(), 1)
        ]
    );

    let (status, json) = send(&app.router, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "favorite_not_found");
}

#[tokio::test]
async fn favorite_of_a_stop_removed_by_a_sync_is_flagged_missing() {
    let (app, server) = synced_app().await;
    create(&app.router, json!({ "stop": "39" })).await;
    create(&app.router, json!({ "stop": "40" })).await;

    // The next sync no longer has stop 40.
    let mut stops = fixture("all_stops.json");
    stops["data"]["stops"]
        .as_array_mut()
        .unwrap()
        .retain(|stop| stop["gtfsId"] != "gtt:40");
    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(stops))
        .mount(&server)
        .await;
    sync_once(&app.stops, &app.upstream, at(NOW + 60))
        .await
        .unwrap();

    let favorites = list(&app.router).await;
    assert_eq!(favorites.len(), 2, "kept, not hidden");
    assert_eq!(favorites[0]["missing"], false);
    assert_eq!(favorites[1]["stop_id"], "gtt:40");
    assert_eq!(favorites[1]["missing"], true);
    assert_eq!(favorites[1]["stop"], Value::Null);
}

#[tokio::test]
async fn unknown_favorites_and_stops_return_404() {
    let (app, _server) = synced_app().await;

    let (status, json) = create(&app.router, json!({ "stop": "99999" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "stop_not_found");

    let (status, json) = send(
        &app.router,
        Method::PATCH,
        "/api/favorites/12345",
        Some(json!({ "label": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "favorite_not_found");
}

#[tokio::test]
async fn creating_before_the_first_sync_returns_503() {
    let app = common::test_app("http://127.0.0.1:9/", at(NOW), Duration::from_secs(1)).await;

    let (status, json) = create(&app.router, json!({ "stop": "39" })).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"]["code"], "index_not_ready");

    let (status, json) = get(&app.router, "/api/favorites").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["favorites"], json!([]));
}

#[tokio::test]
async fn invalid_requests_return_400() {
    let (app, _server) = synced_app().await;
    let (_, json) = create(&app.router, json!({ "stop": "39" })).await;
    let uri = format!("/api/favorites/{}", json["id"]);

    for body in [
        json!({}),
        json!({ "stop": 39 }),
        json!({ "stop": "a b" }),
        json!({ "stop": "39", "label": "x".repeat(101) }),
        json!({ "stop": "39", "lines": "4" }),
        json!({ "stop": "39", "lines": ["12345678901"] }),
        json!({ "stop": "39", "colour": "red" }),
    ] {
        let (status, json) = create(&app.router, body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(json["error"]["code"], "invalid_request", "{body}");
    }

    for (uri, body) in [
        (uri.as_str(), json!({ "position": -1 })),
        (uri.as_str(), json!({ "stop_id": "gtt:40" })),
        ("/api/favorites/abc", json!({ "label": "x" })),
    ] {
        let (status, json) = send(&app.router, Method::PATCH, uri, Some(body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri} {body}");
        assert_eq!(json["error"]["code"], "invalid_request", "{uri} {body}");
    }

    let (status, json) = send(&app.router, Method::DELETE, "/api/favorites/abc", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_request");
}
