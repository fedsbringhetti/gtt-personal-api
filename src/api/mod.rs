use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{
    departures::DeparturesService,
    stops::{Feeds, StopIndex},
    time::Clock,
};

mod error;
mod health;
mod stops;

#[derive(Clone)]
pub struct AppState {
    pub departures: Arc<DeparturesService>,
    pub stops: StopIndex,
    pub default_feeds: Feeds,
    pub clock: Arc<dyn Clock>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/api/stops", get(stops::search))
        .route("/api/stops/nearby", get(stops::nearby))
        .route("/api/stops/{stop}", get(stops::details))
        .route("/api/stops/{stop}/departures", get(stops::departures))
        .with_state(state)
}
