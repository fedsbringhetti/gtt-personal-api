use axum::{Router, routing::get};

mod health;

pub fn router() -> Router {
    Router::new().route("/health", get(health::health))
}
