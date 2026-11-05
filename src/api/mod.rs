use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, patch, post},
};

use crate::{
    departures::DeparturesService,
    favorites::FavoriteStore,
    gtt::{GttRealtime, alerts::AlertsService, static_data::GttStatic},
    rules::RuleStore,
    stops::{Feeds, StopIndex},
    time::Clock,
    upstream::health::UpstreamHealth,
    web::{self, WebAssets},
};

mod alerts;
pub mod auth;
mod error;
mod favorites;
mod health;
mod rules;
mod status;
mod stops;

#[derive(Clone)]
pub struct AppState {
    pub departures: Arc<DeparturesService>,
    pub stops: StopIndex,
    pub favorites: FavoriteStore,
    pub rules: RuleStore,
    pub default_feeds: Feeds,
    pub upstream_health: Arc<UpstreamHealth>,
    /// GTT's real-time data, for `/api/status`; `None` when disabled.
    pub gtt: Option<Arc<GttRealtime>>,
    /// GTT's alerts; `None` when GTT's data is disabled.
    pub alerts: Option<Arc<AlertsService>>,
    /// GTT's static data, for `/api/status`; `None` when disabled.
    pub gtt_static: Option<GttStatic>,
    /// When set, every `/api/*` route requires it as a bearer token.
    pub api_token: Option<auth::ApiToken>,
    pub clock: Arc<dyn Clock>,
    /// The web app, served for every GET that is not an API route.
    pub web: WebAssets,
}

pub fn router(state: AppState) -> Router {
    let mut api = Router::new()
        .route("/api/status", get(status::status))
        .route("/api/alerts", get(alerts::alerts))
        .route("/api/stops", get(stops::search))
        .route("/api/stops/nearby", get(stops::nearby))
        .route("/api/stops/{stop}", get(stops::details))
        .route("/api/stops/{stop}/departures", get(stops::departures))
        .route(
            "/api/favorites",
            get(favorites::list).post(favorites::create),
        )
        .route("/api/favorites/departures", get(favorites::departures))
        .route(
            "/api/favorites/{id}",
            patch(favorites::update).delete(favorites::delete),
        )
        .route("/api/rules", get(rules::list).post(rules::create))
        .route(
            "/api/rules/{id}",
            get(rules::get).patch(rules::update).delete(rules::delete),
        )
        .route("/api/rules/{id}/pause", post(rules::pause))
        .route("/api/rules/{id}/resume", post(rules::resume));
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
