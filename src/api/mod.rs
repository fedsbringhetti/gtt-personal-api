use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, patch},
};

use crate::{
    departures::DeparturesService,
    favorites::FavoriteStore,
    stops::{Feeds, StopIndex},
    time::Clock,
    upstream::health::UpstreamHealth,
    web::{self, WebAssets},
};

pub mod auth;
mod error;
mod favorites;
mod health;
mod status;
mod stops;

#[derive(Clone)]
pub struct AppState {
    pub departures: Arc<DeparturesService>,
    pub stops: StopIndex,
    pub favorites: FavoriteStore,
    pub default_feeds: Feeds,
    pub upstream_health: Arc<UpstreamHealth>,
    /// When set, every `/api/*` route requires it as a bearer token.
    pub api_token: Option<auth::ApiToken>,
    pub clock: Arc<dyn Clock>,
    /// The web app, served for every GET that is not an API route.
    pub web: WebAssets,
}

pub fn router(state: AppState) -> Router {
    let mut api = Router::new()
        .route("/api/status", get(status::status))
        .route("/api/stops", get(stops::search))
        .route("/api/stops/nearby", get(stops::nearby))
        .route("/api/stops/{stop}", get(stops::details))
        .route("/api/stops/{stop}/departures", get(stops::departures))
        .route(
            "/api/favorites",
            get(favorites::list).post(favorites::create),
        )
        .route(
            "/api/favorites/{id}",
            patch(favorites::update).delete(favorites::delete),
        );
    if let Some(token) = state.api_token.clone() {
        api = api.route_layer(middleware::from_fn_with_state(token, auth::require_token));
    }

    // `/health` stays open: the Docker health check has no token. The web app is open too: it
    // holds no data, and every API call it makes goes through the check above.
    let web = state.web;
    Router::new()
        .route("/health", get(health::health))
        .merge(api)
        .with_state(state)
        .fallback_service(get(web::serve).with_state(web))
}
