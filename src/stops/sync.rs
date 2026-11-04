//! Keeps the stop index up to date: on startup when it is missing or older than a day, then
//! every night at a configured local time, retrying with backoff on failure.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Days, NaiveTime, TimeDelta, TimeZone, Utc};

use super::StopIndex;
use crate::{
    time::{Clock, TIME_ZONE},
    upstream::{UpstreamClient, UpstreamError},
};

const MAX_AGE: TimeDelta = TimeDelta::hours(24);
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
];

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Upstream(#[from] UpstreamError),
    #[error("the upstream returned no stops; keeping the current index")]
    Empty,
    #[error(transparent)]
    Index(#[from] super::IndexError),
}

/// Downloads all stops and replaces the index. Returns the number of stops stored.
pub async fn sync_once(
    index: &StopIndex,
    upstream: &UpstreamClient,
    now: DateTime<Utc>,
) -> Result<usize, SyncError> {
    let stops = upstream.all_stops().await?;
    if stops.is_empty() {
        return Err(SyncError::Empty);
    }
    Ok(index.replace_all(stops, now).await?)
}

/// Runs forever in the background.
pub fn spawn(
    index: StopIndex,
    upstream: UpstreamClient,
    clock: Arc<dyn Clock>,
    sync_time: NaiveTime,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let last_sync = match index.last_sync().await {
                Ok(state) => state.map(|state| state.at),
                Err(err) => {
                    tracing::warn!("cannot read the last stop sync: {err}");
                    None
                }
            };
            let now = clock.now();
            if !needs_sync(last_sync, now) {
                let next = next_sync_at(now, sync_time);
                tracing::info!("next stop sync at {}", next.with_timezone(&TIME_ZONE));
                tokio::time::sleep((next - now).to_std().unwrap_or_default()).await;
            }

            let mut attempt = 0;
            loop {
                let started = std::time::Instant::now();
                match sync_once(&index, &upstream, clock.now()).await {
                    Ok(count) => {
                        tracing::info!(
                            "stop index synced: {count} stops in {:?}",
                            started.elapsed()
                        );
                        break;
                    }
                    Err(err) => {
                        let delay = retry_delay(attempt);
                        tracing::warn!("stop sync failed: {err}; retrying in {delay:?}");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                }
            }
        }
    })
}

pub(crate) fn needs_sync(last_sync: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    last_sync.is_none_or(|at| now - at >= MAX_AGE)
}

/// The first `sync_time` (Europe/Rome) strictly after `now`.
pub(crate) fn next_sync_at(now: DateTime<Utc>, sync_time: NaiveTime) -> DateTime<Utc> {
    let today = now.with_timezone(&TIME_ZONE).date_naive();
    [today, today + Days::new(1), today + Days::new(2)]
        .into_iter()
        .filter_map(|day| local_to_utc(day.and_time(sync_time)))
        .find(|at| *at > now)
        .unwrap_or(now + MAX_AGE)
}

/// A local time that does not exist (spring forward) moves one hour later; an ambiguous one
/// (fall back) takes the first occurrence.
fn local_to_utc(local: chrono::NaiveDateTime) -> Option<DateTime<Utc>> {
    TIME_ZONE
        .from_local_datetime(&local)
        .earliest()
        .or_else(|| {
            TIME_ZONE
                .from_local_datetime(&(local + TimeDelta::hours(1)))
                .earliest()
        })
        .map(|at| at.with_timezone(&Utc))
}

pub(crate) fn retry_delay(attempt: usize) -> Duration {
    RETRY_DELAYS[attempt.min(RETRY_DELAYS.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn time(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn needs_sync_when_missing_or_old() {
        let now = utc("2026-10-29T12:00:00+01:00");
        assert!(needs_sync(None, now));
        assert!(needs_sync(Some(now - TimeDelta::hours(25)), now));
        assert!(!needs_sync(Some(now - TimeDelta::hours(23)), now));
    }

    #[test]
    fn next_sync_is_today_or_tomorrow() {
        assert_eq!(
            next_sync_at(utc("2026-10-29T02:00:00+01:00"), time(4, 0)),
            utc("2026-10-29T04:00:00+01:00")
        );
        assert_eq!(
            next_sync_at(utc("2026-10-29T04:00:00+01:00"), time(4, 0)),
            utc("2026-10-30T04:00:00+01:00")
        );
        assert_eq!(
            next_sync_at(utc("2026-10-29T15:00:00+01:00"), time(4, 0)),
            utc("2026-10-30T04:00:00+01:00")
        );
    }

    #[test]
    fn next_sync_handles_daylight_saving_changes() {
        // 02:30 does not exist on 2026-03-29: it moves to 03:30 CEST.
        assert_eq!(
            next_sync_at(utc("2026-03-29T01:00:00+01:00"), time(2, 30)),
            utc("2026-03-29T03:30:00+02:00")
        );
        // 02:30 happens twice on 2026-10-25: the first one (CEST) is used.
        assert_eq!(
            next_sync_at(utc("2026-10-25T01:00:00+02:00"), time(2, 30)),
            utc("2026-10-25T02:30:00+02:00")
        );
        // 04:00 on the spring forward day is already in CEST.
        assert_eq!(
            next_sync_at(utc("2026-03-28T12:00:00+01:00"), time(4, 0)),
            utc("2026-03-29T04:00:00+02:00")
        );
    }

    #[test]
    fn retry_delays_grow_then_stay_at_one_hour() {
        assert_eq!(retry_delay(0), Duration::from_secs(300));
        assert_eq!(retry_delay(1), Duration::from_secs(900));
        assert_eq!(retry_delay(2), Duration::from_secs(3600));
        assert_eq!(retry_delay(10), Duration::from_secs(3600));
    }
}
