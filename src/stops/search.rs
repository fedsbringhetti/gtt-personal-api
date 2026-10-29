use serde::Serialize;
use sqlx::{QueryBuilder, Sqlite};

use super::{Feeds, IndexError, STOP_COLUMNS, StopIndex, StopInfo, StopRow, normalize::normalize};

/// Words shorter than this cannot use the trigram index and are matched with `LIKE`.
const TRIGRAM_LEN: usize = 3;
const EARTH_RADIUS_M: f64 = 6_371_000.0;
const METERS_PER_DEGREE_LAT: f64 = 111_320.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NearbyStop {
    #[serde(flatten)]
    pub stop: StopInfo,
    pub distance_m: u32,
}

impl StopIndex {
    /// Stops matching `query` by code, id, name or description. Ordering: exact code or id,
    /// exact name, name starting with the query, name containing the query, matches in the
    /// description only; then GTT before other feeds; then shorter
    /// names first; then shorter codes (`39` before `252`).
    pub async fn search(
        &self,
        query: &str,
        feeds: &Feeds,
        limit: u32,
    ) -> Result<Vec<StopInfo>, IndexError> {
        let raw = query.trim();
        let normalized = normalize(raw);
        if normalized.is_empty() && raw.is_empty() {
            return Ok(Vec::new());
        }
        let words: Vec<&str> = normalized.split(' ').filter(|w| !w.is_empty()).collect();
        let long_words: Vec<&str> = words
            .iter()
            .copied()
            .filter(|w| w.len() >= TRIGRAM_LEN)
            .collect();
        let short_words: Vec<&str> = words
            .iter()
            .copied()
            .filter(|w| w.len() < TRIGRAM_LEN)
            .collect();

        let mut sql =
            QueryBuilder::<Sqlite>::new(format!("SELECT {STOP_COLUMNS} FROM stops WHERE ("));

        // Exact code or id.
        sql.push("code = ")
            .push_bind(raw)
            .push(" COLLATE NOCASE OR id = ")
            .push_bind(raw);

        // Every word of the query appears in the name or description.
        if !words.is_empty() {
            sql.push(" OR (1 = 1");
            if !long_words.is_empty() {
                // Words are [a-z0-9] only, so quoting them is enough for FTS5.
                let fts_query = long_words
                    .iter()
                    .map(|word| format!("\"{word}\""))
                    .collect::<Vec<_>>()
                    .join(" ");
                sql.push(" AND id IN (SELECT id FROM stops_fts WHERE stops_fts MATCH ")
                    .push_bind(fts_query)
                    .push(")");
            }
            for word in &short_words {
                // Short words must start a word: "po" finds "porta", not "campo".
                sql.push(" AND (' ' || search_text) LIKE ")
                    .push_bind(format!("% {word}%"));
            }
            sql.push(")");
        }
        sql.push(")");

        push_feed_filter(&mut sql, feeds);

        sql.push(" ORDER BY CASE WHEN code = ")
            .push_bind(raw)
            .push(" COLLATE NOCASE OR id = ")
            .push_bind(raw)
            .push(" THEN 0 WHEN search_name = ")
            .push_bind(normalized.clone())
            .push(" THEN 1 WHEN search_name LIKE ")
            .push_bind(format!("{normalized}%"))
            .push(" THEN 2 WHEN instr(search_name, ")
            .push_bind(normalized.clone())
            .push(") > 0 THEN 3 ELSE 4 END, feed <> 'gtt', length(name), name, length(code), code, id LIMIT ")
            .push_bind(limit);

        let rows: Vec<StopRow> = sql.build_query_as().fetch_all(&self.pool).await?;
        rows.into_iter().map(StopInfo::try_from).collect()
    }

    /// Stops within `radius_m` of a point, nearest first.
    pub async fn nearby(
        &self,
        lat: f64,
        lon: f64,
        radius_m: f64,
        feeds: &Feeds,
        limit: usize,
    ) -> Result<Vec<NearbyStop>, IndexError> {
        // A bounding box narrows the candidates using the index; the exact distance decides.
        let delta_lat = radius_m / METERS_PER_DEGREE_LAT;
        let delta_lon = radius_m / (METERS_PER_DEGREE_LAT * lat.to_radians().cos().max(0.01));

        let mut sql = QueryBuilder::<Sqlite>::new(format!(
            "SELECT {STOP_COLUMNS} FROM stops WHERE lat BETWEEN "
        ));
        sql.push_bind(lat - delta_lat)
            .push(" AND ")
            .push_bind(lat + delta_lat)
            .push(" AND lon BETWEEN ")
            .push_bind(lon - delta_lon)
            .push(" AND ")
            .push_bind(lon + delta_lon);
        push_feed_filter(&mut sql, feeds);

        let rows: Vec<StopRow> = sql.build_query_as().fetch_all(&self.pool).await?;
        let mut stops = Vec::with_capacity(rows.len());
        for row in rows {
            let stop = StopInfo::try_from(row)?;
            let distance = distance_m(lat, lon, stop.lat, stop.lon);
            if distance <= radius_m {
                stops.push(NearbyStop {
                    stop,
                    distance_m: distance.round() as u32,
                });
            }
        }
        stops.sort_by(|a, b| {
            a.distance_m
                .cmp(&b.distance_m)
                .then_with(|| a.stop.id.cmp(&b.stop.id))
        });
        stops.truncate(limit);
        Ok(stops)
    }
}

fn push_feed_filter(sql: &mut QueryBuilder<Sqlite>, feeds: &Feeds) {
    if let Feeds::Only(feeds) = feeds {
        sql.push(" AND feed IN (");
        let mut separated = sql.separated(", ");
        for feed in feeds {
            separated.push_bind(feed.clone());
        }
        separated.push_unseparated(")");
    }
}

/// Great-circle distance in meters (haversine formula).
fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (lat1, lat2) = (lat1.to_radians(), lat2.to_radians());
    let d_lat = lat2 - lat1;
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_between_known_points() {
        // Porta Nuova (stop 39) to Porta Susa station: about 1.6 km.
        let d = distance_m(45.06256, 7.67898, 45.07186, 7.66503);
        assert!((1_450.0..1_650.0).contains(&d), "{d}");
        assert_eq!(distance_m(45.0, 7.0, 45.0, 7.0), 0.0);
    }
}
