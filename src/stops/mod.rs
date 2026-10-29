//! Local index of all upstream stops: storage, search and nightly sync.

use std::{cmp::Ordering, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{AssertSqlSafe, QueryBuilder, SqlitePool};

use crate::upstream::types;

pub mod normalize;
mod search;
pub mod sync;

pub use search::NearbyStop;

const SYNC_KEY: &str = "stops";
/// Rows per INSERT; 11 columns each stays well under SQLite's variable limit.
const INSERT_BATCH: usize = 500;

/// Which feeds a search covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feeds {
    All,
    Only(Vec<String>),
}

impl FromStr for Feeds {
    type Err = String;

    /// `all`, or a comma-separated list such as `gtt,2`.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.trim() == "all" {
            return Ok(Self::All);
        }
        let feeds: Vec<String> = value
            .split(',')
            .map(str::trim)
            .filter(|feed| !feed.is_empty())
            .map(str::to_owned)
            .collect();
        if feeds.is_empty() {
            return Err("expected \"all\" or a comma-separated list of feeds".to_owned());
        }
        if let Some(feed) = feeds
            .iter()
            .find(|feed| !feed.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err(format!("invalid feed {feed:?}"));
        }
        Ok(Self::Only(feeds))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineRef {
    pub id: String,
    pub name: String,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StopInfo {
    pub id: String,
    pub feed: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub mode: Option<String>,
    pub lines: Vec<LineRef>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncState {
    pub at: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid stored lines: {0}")]
    Lines(#[from] serde_json::Error),
}

/// A stop ready to be stored, with its normalized search text.
#[derive(Debug, Clone)]
struct NewStop {
    info: StopInfo,
    search_name: String,
    search_text: String,
}

#[derive(sqlx::FromRow)]
struct StopRow {
    id: String,
    feed: String,
    code: String,
    name: String,
    description: Option<String>,
    lat: f64,
    lon: f64,
    mode: Option<String>,
    lines: String,
}

impl TryFrom<StopRow> for StopInfo {
    type Error = IndexError;

    fn try_from(row: StopRow) -> Result<Self, Self::Error> {
        Ok(Self {
            lines: serde_json::from_str(&row.lines)?,
            id: row.id,
            feed: row.feed,
            code: row.code,
            name: row.name,
            description: row.description,
            lat: row.lat,
            lon: row.lon,
            mode: row.mode,
        })
    }
}

const STOP_COLUMNS: &str = "id, feed, code, name, description, lat, lon, mode, lines";

#[derive(Debug, Clone)]
pub struct StopIndex {
    pool: SqlitePool,
}

impl StopIndex {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// `None` until the first successful sync.
    pub async fn last_sync(&self) -> Result<Option<SyncState>, IndexError> {
        let row: Option<(DateTime<Utc>, i64)> =
            sqlx::query_as("SELECT at, count FROM sync_state WHERE key = ?")
                .bind(SYNC_KEY)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(at, count)| SyncState { at, count }))
    }

    pub async fn get(&self, id: &str) -> Result<Option<StopInfo>, IndexError> {
        let row: Option<StopRow> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {STOP_COLUMNS} FROM stops WHERE id = ?"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(StopInfo::try_from).transpose()
    }

    /// Replaces the whole index in one transaction: readers see either the old or the new data.
    pub async fn replace_all(
        &self,
        stops: Vec<types::IndexStop>,
        at: DateTime<Utc>,
    ) -> Result<usize, IndexError> {
        let stops: Vec<NewStop> = stops.into_iter().map(NewStop::from_upstream).collect();
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM stops").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM stops_fts")
            .execute(&mut *tx)
            .await?;

        for batch in stops.chunks(INSERT_BATCH) {
            let lines = batch
                .iter()
                .map(|stop| serde_json::to_string(&stop.info.lines))
                .collect::<Result<Vec<_>, _>>()?;
            let mut insert = QueryBuilder::new(format!(
                "INSERT INTO stops ({STOP_COLUMNS}, search_name, search_text) "
            ));
            insert.push_values(batch.iter().zip(&lines), |mut row, (stop, lines)| {
                row.push_bind(&stop.info.id)
                    .push_bind(&stop.info.feed)
                    .push_bind(&stop.info.code)
                    .push_bind(&stop.info.name)
                    .push_bind(&stop.info.description)
                    .push_bind(stop.info.lat)
                    .push_bind(stop.info.lon)
                    .push_bind(&stop.info.mode)
                    .push_bind(lines)
                    .push_bind(&stop.search_name)
                    .push_bind(&stop.search_text);
            });
            insert.build().execute(&mut *tx).await?;
        }

        sqlx::query("INSERT INTO stops_fts (id, text) SELECT id, search_text FROM stops")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO sync_state (key, at, count) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET at = excluded.at, count = excluded.count",
        )
        .bind(SYNC_KEY)
        .bind(at)
        .bind(stops.len() as i64)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(stops.len())
    }
}

impl NewStop {
    fn from_upstream(stop: types::IndexStop) -> Self {
        let feed = stop
            .gtfs_id
            .split_once(':')
            .map_or("", |(feed, _)| feed)
            .to_owned();
        let code = display_code(stop.code, &stop.gtfs_id);
        let name = stop.name.unwrap_or_default();
        let lines = lines_from_routes(stop.routes.unwrap_or_default());
        let mode = stop
            .vehicle_mode
            .or_else(|| lines.iter().find_map(|line| line.mode.clone()));

        let search_name = normalize::normalize(&name);
        let search_text = match &stop.desc {
            Some(desc) => format!("{search_name} {}", normalize::normalize(desc)),
            None => search_name.clone(),
        };

        Self {
            info: StopInfo {
                id: stop.gtfs_id,
                feed,
                code,
                name,
                description: stop.desc,
                lat: stop.lat,
                lon: stop.lon,
                mode,
                lines,
            },
            search_name,
            search_text,
        }
    }
}

/// The code shown to the user: the upstream code, or the part of the id after `feed:` when the
/// upstream has none (GTT metro stops).
pub fn display_code(code: Option<String>, id: &str) -> String {
    match code {
        Some(code) if !code.is_empty() => code,
        _ => id.split_once(':').map_or(id, |(_, code)| code).to_owned(),
    }
}

/// One entry per line name, sorted naturally (`4`, `9`, `9B`, `15`, `M1S`).
fn lines_from_routes(routes: Vec<types::RouteRef>) -> Vec<LineRef> {
    let mut lines: Vec<LineRef> = Vec::new();
    for route in routes {
        let Some(name) = route.short_name.filter(|name| !name.is_empty()) else {
            continue;
        };
        if lines.iter().all(|line| line.name != name) {
            lines.push(LineRef {
                id: route.gtfs_id,
                name,
                mode: route.mode,
            });
        }
    }
    lines.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    lines
}

/// Compares the leading number first, then the rest: `9` < `9B` < `15` < `M1S`.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn split(s: &str) -> (Option<u32>, &str) {
        let digits = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        (s[..digits].parse().ok(), &s[digits..])
    }
    let (a_number, a_rest) = split(a);
    let (b_number, b_rest) = split(b);
    match (a_number, b_number) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a_rest.cmp(b_rest)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_feeds() {
        assert_eq!("all".parse(), Ok(Feeds::All));
        assert_eq!(
            "gtt, 2".parse(),
            Ok(Feeds::Only(vec!["gtt".to_owned(), "2".to_owned()]))
        );
        assert!("".parse::<Feeds>().is_err());
        assert!(" , ".parse::<Feeds>().is_err());
        assert!("gtt;2".parse::<Feeds>().is_err());
    }

    #[test]
    fn display_code_falls_back_to_id() {
        assert_eq!(display_code(Some("39".into()), "gtt:39"), "39");
        assert_eq!(display_code(None, "gtt:8224"), "8224");
        assert_eq!(display_code(Some(String::new()), "gtt:8224"), "8224");
    }

    #[test]
    fn sorts_lines_naturally() {
        let mut names = vec!["M1S", "68+", "9B", "15", "4", "9", "68", "33"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["4", "9", "9B", "15", "33", "68", "68+", "M1S"]);
    }

    #[test]
    fn deduplicates_lines_by_name() {
        let route = |id: &str, name: Option<&str>| types::RouteRef {
            gtfs_id: id.to_owned(),
            short_name: name.map(str::to_owned),
            mode: Some("BUS".to_owned()),
        };
        let lines = lines_from_routes(vec![
            route("gtt:15U", Some("15")),
            route("gtt:4U", Some("4")),
            route("gtt:15E", Some("15")),
            route("gtt:x", None),
        ]);
        let names: Vec<_> = lines.iter().map(|line| line.name.as_str()).collect();
        assert_eq!(names, ["4", "15"]);
        assert_eq!(lines[1].id, "gtt:15U");
    }
}
