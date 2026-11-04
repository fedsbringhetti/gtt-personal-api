//! GTT's GTFS static archive, of which only `stops.txt` is used: it links GTT's stop ids, used
//! by the alerts, to our stop ids (Phase 3, decision Q).
//!
//! The archive is about 20 MB and GTT rebuilds it about once a week. It is checked every night
//! with a conditional request: GTT answers `304 Not Modified` with no body when it has not
//! changed since the `Last-Modified` stored at the last download.

use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use reqwest::{StatusCode, header};
use serde::Serialize;
use sqlx::{QueryBuilder, SqlitePool};

use crate::{
    stops::sync::{needs_sync, next_sync_at, retry_delay},
    time::{Clock, TIME_ZONE},
    upstream::{UpstreamError, health::UpstreamHealth},
};

const SYNC_KEY: &str = "gtt_static";
/// The archive is large: it gets much more time than the real-time feeds.
const TIMEOUT: Duration = Duration::from_secs(120);
/// Our ids are GTT's stop codes in the `gtt` feed.
const FEED_PREFIX: &str = "gtt:";
/// Rows per INSERT; 2 columns each.
const INSERT_BATCH: usize = 1_000;
/// The nightly check runs this long after the stop index sync, so the two do not overlap.
pub const CHECK_AFTER_STOPS_SYNC: TimeDelta = TimeDelta::minutes(10);

#[derive(Debug, thiserror::Error)]
pub enum StaticError {
    #[error(transparent)]
    Download(#[from] UpstreamError),
    #[error("invalid GTFS archive: {0}")]
    Archive(String),
    #[error("stops.txt has no stops; keeping the current ones")]
    Empty,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// What the stored static data is, for `/api/status`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StaticState {
    /// When the archive was last checked, whether it had changed or not.
    pub checked_at: DateTime<Utc>,
    /// The archive's `Last-Modified`, when GTT sent one.
    pub version: Option<DateTime<Utc>>,
    pub stops: i64,
}

/// What a check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// GTT answered 304: the stored data is current.
    Unchanged,
    /// A new archive was read; the number of stops stored.
    Updated(usize),
}

/// GTT's static data: download, storage and lookup.
#[derive(Clone)]
pub struct GttStatic {
    pool: SqlitePool,
    http: reqwest::Client,
    url: reqwest::Url,
    health: Arc<UpstreamHealth>,
}

impl GttStatic {
    pub fn new(pool: SqlitePool, url: reqwest::Url) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self {
            pool,
            http,
            url,
            health: Arc::default(),
        })
    }

    pub fn health(&self) -> Arc<UpstreamHealth> {
        self.health.clone()
    }

    /// `None` until the first successful download.
    pub async fn state(&self) -> Result<Option<StaticState>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, i64, Option<String>)> =
            sqlx::query_as("SELECT at, count, version FROM sync_state WHERE key = ?")
                .bind(SYNC_KEY)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(checked_at, stops, version)| StaticState {
            checked_at,
            version: version.as_deref().and_then(parse_http_date),
            stops,
        }))
    }

    /// GTT's stop id → our stop id, for linking alerts. Empty before the first download.
    pub async fn stop_ids(&self) -> Result<HashMap<String, String>, sqlx::Error> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT gtt_stop_id, stop_id FROM gtt_stops")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    /// Asks GTT for the archive, unless it has not changed since the stored version, and
    /// replaces the stored stops when it has.
    pub async fn check(&self, now: DateTime<Utc>) -> Result<CheckOutcome, StaticError> {
        let result = self.check_inner(now).await;
        match &result {
            Ok(_) => self.health.record_success(now),
            Err(StaticError::Download(err)) => self.health.record_failure(now, err),
            Err(err) => self
                .health
                .record_failure(now, &UpstreamError::InvalidResponse(err.to_string())),
        }
        result
    }

    async fn check_inner(&self, now: DateTime<Utc>) -> Result<CheckOutcome, StaticError> {
        let stored: Option<String> =
            sqlx::query_scalar("SELECT version FROM sync_state WHERE key = ?")
                .bind(SYNC_KEY)
                .fetch_optional(&self.pool)
                .await?
                .flatten();

        let mut request = self.http.get(self.url.clone());
        if let Some(version) = &stored {
            request = request.header(header::IF_MODIFIED_SINCE, version);
        }
        let response = request.send().await.map_err(UpstreamError::from_reqwest)?;
        let status = response.status();
        if status == StatusCode::NOT_MODIFIED {
            sqlx::query("UPDATE sync_state SET at = ? WHERE key = ?")
                .bind(now)
                .bind(SYNC_KEY)
                .execute(&self.pool)
                .await?;
            return Ok(CheckOutcome::Unchanged);
        }
        if !status.is_success() {
            return Err(UpstreamError::Status(status.as_u16()).into());
        }
        let version = response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let archive = response
            .bytes()
            .await
            .map_err(UpstreamError::from_reqwest)?;

        // Unzipping and parsing take a few milliseconds of CPU: off the async threads.
        let stops = tokio::task::spawn_blocking(move || read_stops(&archive))
            .await
            .map_err(|err| StaticError::Archive(err.to_string()))??;
        if stops.is_empty() {
            return Err(StaticError::Empty);
        }
        self.replace(&stops, version.as_deref(), now).await?;
        Ok(CheckOutcome::Updated(stops.len()))
    }

    /// Replaces every stop in one transaction: alerts linked meanwhile see the old or the new
    /// data, never a mix.
    async fn replace(
        &self,
        stops: &[(String, String)],
        version: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM gtt_stops")
            .execute(&mut *tx)
            .await?;
        for batch in stops.chunks(INSERT_BATCH) {
            let mut insert = QueryBuilder::new("INSERT INTO gtt_stops (gtt_stop_id, stop_id) ");
            insert.push_values(batch, |mut row, (gtt_stop_id, stop_id)| {
                row.push_bind(gtt_stop_id).push_bind(stop_id);
            });
            insert.build().execute(&mut *tx).await?;
        }
        sqlx::query(
            "INSERT INTO sync_state (key, at, count, version) VALUES (?, ?, ?, ?)
             ON CONFLICT (key) DO UPDATE
             SET at = excluded.at, count = excluded.count, version = excluded.version",
        )
        .bind(SYNC_KEY)
        .bind(now)
        .bind(stops.len() as i64)
        .bind(version)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
}

/// `(GTT stop_id, our stop id)` for every stop in the archive's `stops.txt`. Stations
/// (`location_type` 1) and rows without a code are skipped: no departure stops there.
pub fn read_stops(archive: &[u8]) -> Result<Vec<(String, String)>, StaticError> {
    let invalid = |err: &dyn std::fmt::Display| StaticError::Archive(err.to_string());
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|err| invalid(&err))?;
    let mut text = String::new();
    zip.by_name("stops.txt")
        .map_err(|err| invalid(&err))?
        .read_to_string(&mut text)
        .map_err(|err| invalid(&err))?;

    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|err| invalid(&err))?.clone();
    let column = |name: &str| headers.iter().position(|header| header == name);
    let (Some(id), Some(code)) = (column("stop_id"), column("stop_code")) else {
        return Err(StaticError::Archive(
            "stops.txt has no stop_id or stop_code column".to_owned(),
        ));
    };
    let location_type = column("location_type");

    let mut stops = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|err| invalid(&err))?;
        let field = |index: usize| record.get(index).unwrap_or("").trim();
        let is_station = location_type.is_some_and(|index| field(index) == "1");
        if is_station || field(id).is_empty() || field(code).is_empty() {
            continue;
        }
        stops.push((
            field(id).to_owned(),
            format!("{FEED_PREFIX}{}", field(code)),
        ));
    }
    Ok(stops)
}

/// An HTTP date (`Sun, 01 Nov 2026 02:26:59 GMT`), the format of `Last-Modified`.
fn parse_http_date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

/// Runs forever in the background: a check at startup when the data is missing or was not
/// checked for a day, then every night at `check_time` (Europe/Rome), retrying with backoff on
/// failure. `on_update` runs after each new download.
pub fn spawn(
    gtt_static: GttStatic,
    clock: Arc<dyn Clock>,
    check_time: NaiveTime,
    on_update: impl Fn() + Send + 'static,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let last_check = match gtt_static.state().await {
                Ok(state) => state.map(|state| state.checked_at),
                Err(err) => {
                    tracing::warn!("cannot read the last GTT static data check: {err}");
                    None
                }
            };
            let now = clock.now();
            if !needs_sync(last_check, now) {
                let next = next_sync_at(now, check_time);
                tracing::info!(
                    "next GTT static data check at {}",
                    next.with_timezone(&TIME_ZONE)
                );
                tokio::time::sleep((next - now).to_std().unwrap_or_default()).await;
            }

            let mut attempt = 0;
            loop {
                match gtt_static.check(clock.now()).await {
                    Ok(CheckOutcome::Unchanged) => {
                        tracing::info!("GTT static data unchanged");
                        break;
                    }
                    Ok(CheckOutcome::Updated(count)) => {
                        tracing::info!("GTT static data downloaded: {count} stops");
                        on_update();
                        break;
                    }
                    Err(err) => {
                        let delay = retry_delay(attempt);
                        tracing::warn!(
                            "GTT static data check failed: {err}; retrying in {delay:?}"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// A zip with the given files, as GTT builds it (deflate).
    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    const STOPS: &str = r#""stop_id","stop_code","stop_name","stop_desc","stop_lat","stop_lon","zone_id","stop_url","location_type","parent_station","stop_timezone","wheelchair_boarding"
"2843","39","Fermata 39 - PORTA NUOVA","PORTA NUOVA - M1 - STAZIONE","45.06256","7.67898","0","","","","Europe/Rome","1"
"3634","8224","METRO PORTA NUOVA","PORTA NUOVA M1","45.06276","7.679","7","","","116589","","1"
"116589","PORTANUOVA","STAZIONE PORTA NUOVA","","45.0626","7.6790","","","1","","",""
"9999","","NO CODE","","45.0","7.6","","","","","",""
"#;

    #[test]
    fn reads_stop_ids_and_skips_stations() {
        let zip = archive(&[("agency.txt", "agency_id\nU\n"), ("stops.txt", STOPS)]);
        assert_eq!(
            read_stops(&zip).unwrap(),
            [
                ("2843".to_owned(), "gtt:39".to_owned()),
                ("3634".to_owned(), "gtt:8224".to_owned()),
            ]
        );
    }

    #[test]
    fn rejects_archives_without_stops() {
        let zip = archive(&[("agency.txt", "agency_id\nU\n")]);
        assert!(matches!(read_stops(&zip), Err(StaticError::Archive(_))));
        assert!(matches!(
            read_stops(b"not a zip"),
            Err(StaticError::Archive(_))
        ));
        let zip = archive(&[("stops.txt", "stop_name\nPORTA NUOVA\n")]);
        assert!(matches!(read_stops(&zip), Err(StaticError::Archive(_))));
    }

    #[test]
    fn parses_last_modified() {
        assert_eq!(
            parse_http_date("Sun, 01 Nov 2026 02:26:59 GMT"),
            DateTime::from_timestamp(1_793_500_019, 0)
        );
        assert_eq!(parse_http_date("yesterday"), None);
    }
}
