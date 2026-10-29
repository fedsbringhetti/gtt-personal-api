//! Favorite stops, stored in SQLite with a manual order.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Favorite {
    pub id: i64,
    pub stop_id: String,
    pub label: Option<String>,
    /// Empty means all lines.
    pub lines: Vec<String>,
    /// 0-based; the list is always contiguous.
    pub position: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewFavorite {
    pub stop_id: String,
    pub label: Option<String>,
    pub lines: Vec<String>,
}

/// Fields to change; `None` leaves the field as it is.
#[derive(Debug, Clone, Default)]
pub struct FavoriteUpdate {
    /// `Some(None)` clears the label.
    pub label: Option<Option<String>>,
    pub lines: Option<Vec<String>>,
    /// New 0-based position; values past the end move the favorite to the end.
    pub position: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum FavoritesError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid stored lines: {0}")]
    Lines(#[from] serde_json::Error),
}

#[derive(sqlx::FromRow)]
struct FavoriteRow {
    id: i64,
    stop_id: String,
    label: Option<String>,
    lines: String,
    position: i64,
    created_at: DateTime<Utc>,
}

impl TryFrom<FavoriteRow> for Favorite {
    type Error = FavoritesError;

    fn try_from(row: FavoriteRow) -> Result<Self, Self::Error> {
        Ok(Self {
            lines: serde_json::from_str(&row.lines)?,
            id: row.id,
            stop_id: row.stop_id,
            label: row.label,
            position: row.position,
            created_at: row.created_at,
        })
    }
}

const SELECT_FAVORITE: &str =
    "SELECT id, stop_id, label, lines, position, created_at FROM favorites";

#[derive(Debug, Clone)]
pub struct FavoriteStore {
    pool: SqlitePool,
}

impl FavoriteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// All favorites in their manual order.
    pub async fn list(&self) -> Result<Vec<Favorite>, FavoritesError> {
        let rows: Vec<FavoriteRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT_FAVORITE} ORDER BY position"
        )))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(Favorite::try_from).collect()
    }

    pub async fn get(&self, id: i64) -> Result<Option<Favorite>, FavoritesError> {
        let row: Option<FavoriteRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT_FAVORITE} WHERE id = ?"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(Favorite::try_from).transpose()
    }

    /// Adds a favorite at the end of the list.
    pub async fn create(
        &self,
        favorite: NewFavorite,
        now: DateTime<Utc>,
    ) -> Result<Favorite, FavoritesError> {
        let lines = serde_json::to_string(&favorite.lines)?;
        let mut tx = self.pool.begin().await?;
        let (position,): (i64,) =
            sqlx::query_as("SELECT COALESCE(MAX(position) + 1, 0) FROM favorites")
                .fetch_one(&mut *tx)
                .await?;
        let id = sqlx::query(
            "INSERT INTO favorites (stop_id, label, lines, position, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&favorite.stop_id)
        .bind(&favorite.label)
        .bind(lines)
        .bind(position)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
        tx.commit().await?;

        Ok(Favorite {
            id,
            stop_id: favorite.stop_id,
            label: favorite.label,
            lines: favorite.lines,
            position,
            created_at: now,
        })
    }

    /// `None` when the favorite does not exist.
    pub async fn update(
        &self,
        id: i64,
        update: FavoriteUpdate,
    ) -> Result<Option<Favorite>, FavoritesError> {
        let mut tx = self.pool.begin().await?;
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM favorites WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        if exists.is_none() {
            return Ok(None);
        }

        if let Some(label) = update.label {
            sqlx::query("UPDATE favorites SET label = ? WHERE id = ?")
                .bind(label)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(lines) = update.lines {
            sqlx::query("UPDATE favorites SET lines = ? WHERE id = ?")
                .bind(serde_json::to_string(&lines)?)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(position) = update.position {
            let ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM favorites ORDER BY position")
                .fetch_all(&mut *tx)
                .await?;
            let ids: Vec<i64> = ids.into_iter().map(|(id,)| id).collect();
            for (position, id) in move_to(&ids, id, position).into_iter().enumerate() {
                sqlx::query("UPDATE favorites SET position = ? WHERE id = ?")
                    .bind(position as i64)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        tx.commit().await?;
        self.get(id).await
    }

    /// `false` when the favorite does not exist. The following favorites move up one place.
    pub async fn delete(&self, id: i64) -> Result<bool, FavoritesError> {
        let mut tx = self.pool.begin().await?;
        let deleted: Option<(i64,)> =
            sqlx::query_as("DELETE FROM favorites WHERE id = ? RETURNING position")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((position,)) = deleted else {
            return Ok(false);
        };
        sqlx::query("UPDATE favorites SET position = position - 1 WHERE position > ?")
            .bind(position)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
}

/// `ids` with `id` moved to `position` (clamped to the end).
fn move_to(ids: &[i64], id: i64, position: usize) -> Vec<i64> {
    let mut ids: Vec<i64> = ids.iter().copied().filter(|other| *other != id).collect();
    ids.insert(position.min(ids.len()), id);
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_to_reorders_and_clamps() {
        assert_eq!(move_to(&[1, 2, 3], 3, 0), [3, 1, 2]);
        assert_eq!(move_to(&[1, 2, 3], 1, 2), [2, 3, 1]);
        assert_eq!(move_to(&[1, 2, 3], 2, 1), [1, 2, 3]);
        assert_eq!(move_to(&[1, 2, 3], 1, 99), [2, 3, 1]);
    }
}
