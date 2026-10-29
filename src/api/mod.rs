use std::sync::Arc;

use axum::{Router, routing::get};

use crate::{departures::DeparturesService, time::Clock};

mod error;
mod health;
mod stops;

#[derive(Clone)]
pub struct AppState {
    pub departures: Arc<DeparturesService>,
    pub clock: Arc<dyn Clock>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/api/stops/{stop}/departures", get(stops::departures))
        .with_state(state)
}
