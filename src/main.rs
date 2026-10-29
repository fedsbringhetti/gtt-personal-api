use std::{process::ExitCode, sync::Arc};

use gtt_personal_api::{
    api::{self, AppState},
    config::Config,
    db,
    departures::DeparturesService,
    favorites::FavoriteStore,
    stops::{StopIndex, sync},
    time::{Clock, SystemClock},
    upstream::UpstreamClient,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("configuration error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let upstream = match UpstreamClient::new(config.upstream_url.clone(), config.upstream_timeout) {
        Ok(upstream) => upstream,
        Err(err) => {
            tracing::error!("cannot create the upstream client: {err}");
            return ExitCode::FAILURE;
        }
    };
    let pool = match db::connect(&config.data_dir).await {
        Ok(pool) => pool,
        Err(err) => {
            tracing::error!(
                "cannot open the database in {}: {err}",
                config.data_dir.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let stops = StopIndex::new(pool.clone());
    let favorites = FavoriteStore::new(pool);
    sync::spawn(
        stops.clone(),
        upstream.clone(),
        clock.clone(),
        config.stops_sync_time,
    );

    if config.api_token.is_none() {
        tracing::warn!("API_TOKEN is not set: /api/* is open to anyone who can reach the service");
    }
    let upstream_health = upstream.health();
    let state = AppState {
        departures: Arc::new(DeparturesService::new(
            upstream,
            config.departures_cache_ttl,
            clock.clone(),
        )),
        stops,
        favorites,
        default_feeds: config.default_feeds.clone(),
        upstream_health,
        api_token: config.api_token.clone(),
        clock,
    };

    let listener = match TcpListener::bind(config.bind_addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("cannot bind {}: {err}", config.bind_addr);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on {}", config.bind_addr);

    if let Err(err) = axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("server error: {err}");
        return ExitCode::FAILURE;
    }

    tracing::info!("shut down");
    ExitCode::SUCCESS
}

/// Resolves on Ctrl+C or SIGTERM (what `docker stop` sends).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::warn!("cannot listen for SIGTERM: {err}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
