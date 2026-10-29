mod common;

use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};

use common::{NOW, Options, at, request};

const TOKEN: &str = "s3cret-token";

async fn app_with_token() -> axum::Router {
    common::test_app_with(
        "http://127.0.0.1:9/",
        at(NOW),
        Duration::from_secs(1),
        Options {
            api_token: Some(TOKEN),
            ..Default::default()
        },
    )
    .await
    .router
}

fn get(uri: &str, authorization: Option<&str>) -> Request<Body> {
    let mut builder = Request::get(uri);
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn api_requires_the_token_when_configured() {
    let app = app_with_token().await;

    for authorization in [
        None,
        Some("Bearer wrong"),
        Some("Bearer s3cret-toke"),
        Some("Basic s3cret-token"),
        Some("s3cret-token"),
    ] {
        let (status, headers, json) = request(&app, get("/api/favorites", authorization)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{authorization:?}");
        assert_eq!(json["error"]["code"], "unauthorized");
        assert_eq!(headers[header::WWW_AUTHENTICATE], "Bearer");
    }

    for authorization in ["Bearer s3cret-token", "bearer s3cret-token"] {
        let (status, _, _) = request(&app, get("/api/favorites", Some(authorization))).await;
        assert_eq!(status, StatusCode::OK, "{authorization}");
    }
}

#[tokio::test]
async fn health_stays_open() {
    let app = app_with_token().await;

    let (status, _, json) = request(&app, get("/health", None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn no_token_configured_means_open_api() {
    let app = common::test_app("http://127.0.0.1:9/", at(NOW), Duration::from_secs(1))
        .await
        .router;

    let (status, _, _) = request(&app, get("/api/favorites", None)).await;

    assert_eq!(status, StatusCode::OK);
}
