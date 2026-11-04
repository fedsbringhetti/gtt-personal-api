//! GTT's own open data: real-time trip updates for urban runs (Phase 3, decision N), service
//! alerts (decision O) and the static data that links alerts to stops (decision Q).
//!
//! The feeds are plain HTTP GETs returning GTFS Realtime messages (Protocol Buffers). Every
//! response is generated on request, so it differs byte by byte even when nothing changed:
//! freshness comes from each run's own timestamp, never from the header.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use moka::future::Cache;
use prost::Message;
use serde::Serialize;

use crate::upstream::{UpstreamError, health::UpstreamHealth};

pub mod alerts;
pub mod comparison;
#[rustfmt::skip]
#[allow(clippy::all, clippy::pedantic, rustdoc::all, missing_docs)]
pub mod proto;
pub mod static_data;
pub mod trip_updates;

use comparison::Comparisons;
use trip_updates::TripUpdates;

/// GTT answers in well under a second; departures wait for it, so it gets a short timeout.
const TIMEOUT: Duration = Duration::from_secs(5);
/// Wait before the single retry of a transient failure, as for OpenTripPlanner.
const RETRY_DELAY: Duration = Duration::from_millis(300);
/// After a failure GTT is left alone for this long, so departures do not wait for it at
/// every request while it is down.
const PAUSE_AFTER_FAILURE: Duration = Duration::from_secs(30);

/// GTT's real-time data, read on demand and cached.
pub struct GttRealtime {
    http: reqwest::Client,
    trip_updates_url: reqwest::Url,
    alerts_url: reqwest::Url,
    cache: Cache<(), Arc<TripUpdates>>,
    health: Arc<UpstreamHealth>,
    alerts_health: Arc<UpstreamHealth>,
    paused_until: Mutex<Option<Instant>>,
    latest: Mutex<Option<FeedSummary>>,
    /// Today's comparison with OpenTripPlanner, filled while merging departures.
    pub comparisons: Comparisons,
}

/// The last trip updates read, for `/api/status`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FeedSummary {
    pub runs: usize,
    /// The most recent update of any run.
    pub newest_update: Option<DateTime<Utc>>,
}

impl GttRealtime {
    /// `base_url` is the folder of the feeds, e.g. `https://percorsieorari.gtt.to.it/das_gtfsrt/`.
    pub fn new(base_url: &reqwest::Url, cache_ttl: Duration) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        // Without a final `/`, joining would replace the last folder instead of adding to it.
        let mut base_url = base_url.clone();
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let feed = |name: &str| base_url.join(name).expect("a relative path always joins");
        Ok(Self {
            http,
            trip_updates_url: feed("trip_update.aspx"),
            alerts_url: feed("alerts.aspx"),
            cache: Cache::builder().time_to_live(cache_ttl).build(),
            health: Arc::default(),
            alerts_health: Arc::default(),
            paused_until: Mutex::new(None),
            latest: Mutex::new(None),
            comparisons: Comparisons::default(),
        })
    }

    /// Outcome of the recent reads of the trip updates.
    pub fn health(&self) -> Arc<UpstreamHealth> {
        self.health.clone()
    }

    /// Outcome of the recent reads of the alerts.
    pub fn alerts_health(&self) -> Arc<UpstreamHealth> {
        self.alerts_health.clone()
    }

    pub fn latest(&self) -> Option<FeedSummary> {
        *lock(&self.latest)
    }

    /// The current trip updates, from the cache when fresh. `None` when GTT cannot be read: the
    /// failure is logged and recorded, and departures carry on with OpenTripPlanner alone.
    pub async fn trip_updates(&self) -> Option<Arc<TripUpdates>> {
        if let Some(updates) = self.cache.get(&()).await {
            return Some(updates);
        }
        if lock(&self.paused_until).is_some_and(|until| Instant::now() < until) {
            return None;
        }
        // Concurrent requests share one call; errors are not cached.
        match self.cache.try_get_with((), self.fetch()).await {
            Ok(updates) => Some(updates),
            Err(err) => {
                tracing::warn!("GTT trip updates unavailable: {err}");
                *lock(&self.paused_until) = Some(Instant::now() + PAUSE_AFTER_FAILURE);
                None
            }
        }
    }

    async fn fetch(&self) -> Result<Arc<TripUpdates>, UpstreamError> {
        let result = self.read(&self.trip_updates_url).await.and_then(|body| {
            TripUpdates::decode(&body)
                .map_err(|err| UpstreamError::InvalidResponse(err.to_string()))
        });
        match &result {
            Ok(updates) => {
                self.health.record_success(Utc::now());
                *lock(&self.latest) = Some(FeedSummary {
                    runs: updates.len(),
                    newest_update: updates.newest_update,
                });
            }
            Err(err) => self.health.record_failure(Utc::now(), err),
        }
        result.map(Arc::new)
    }

    /// The current alerts feed, read now (the caller keeps it: alerts are read every few
    /// minutes, not on demand).
    pub async fn alerts(&self) -> Result<proto::FeedMessage, UpstreamError> {
        let result = self.read(&self.alerts_url).await.and_then(|body| {
            proto::FeedMessage::decode(body.as_slice())
                .map_err(|err| UpstreamError::InvalidResponse(err.to_string()))
        });
        match &result {
            Ok(_) => self.alerts_health.record_success(Utc::now()),
            Err(err) => self.alerts_health.record_failure(Utc::now(), err),
        }
        result
    }

    /// One feed's body, with a single retry after a transient failure.
    async fn read(&self, url: &reqwest::Url) -> Result<Vec<u8>, UpstreamError> {
        let mut result = self.read_once(url).await;
        if let Err(err) = &result
            && err.is_transient()
        {
            tracing::debug!("retrying after transient GTT failure: {err}");
            tokio::time::sleep(RETRY_DELAY).await;
            result = self.read_once(url).await;
        }
        result
    }

    async fn read_once(&self, url: &reqwest::Url) -> Result<Vec<u8>, UpstreamError> {
        let response = self
            .http
            .get(url.clone())
            .send()
            .await
            .map_err(UpstreamError::from_reqwest)?;
        let status = response.status();
        if !status.is_success() {
            return Err(UpstreamError::Status(status.as_u16()));
        }
        let body = response
            .bytes()
            .await
            .map_err(UpstreamError::from_reqwest)?;
        Ok(body.into())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // The values behind these locks are replaced whole, so a panic cannot leave them broken.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
