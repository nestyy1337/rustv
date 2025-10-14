use chrono::DateTime;
use sqlx::{Pool, Sqlite};

use crate::models::movie::Watchlist;

pub struct WatchlistRepository;

impl WatchlistRepository {
    pub async fn find_by_user_id(
        pool: &Pool<Sqlite>,
        user_id: i64,
    ) -> Result<Vec<Watchlist>, sqlx::Error> {
        // SELECT wm.id, m.imdb_id, wm.user_id, wm.movie_id, m.title, m.genre, m.release_year as year, wm.watched_at, wm.rating
        // FROM watched_movies wm
        // INNER JOIN movies m ON wm.movie_id = m.id
        // WHERE wm.user_id = ?
        // ORDER BY wm.watched_at DESC
        sqlx::query_as!(
            Watchlist,
            r#"SELECT w.id as "id!", w.user_id as "user_id!", w.movie_id as "movie_id!", m.available as available, w.added_at FROM watchlist w
            INNER JOIN movies m ON w.movie_id = m.id
            WHERE user_id = ?"#,
            user_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete(
        pool: &Pool<Sqlite>,
        user_id: i64,
        movie_id: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM watchlist WHERE user_id = ? AND movie_id = ?",
            user_id,
            movie_id
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    // CREATE TABLE IF NOT EXISTS watchlist (
    //     id INTEGER PRIMARY KEY AUTOINCREMENT,
    //     user_id INTEGER NOT NULL,
    //     movie_id INTEGER NOT NULL,
    //     added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    //     FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    //     FOREIGN KEY (movie_id) REFERENCES movies(id) ON DELETE CASCADE,
    //     UNIQUE(user_id, movie_id)

    pub async fn add(pool: &Pool<Sqlite>, user_id: i64, movie_id: i64) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();

        let _ = sqlx::query!(
            "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?,?,?) ON CONFLICT(user_id, movie_id) DO NOTHING",
            user_id,
            movie_id,
            now
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn is_watchlisted_anywhere(
        pool: &Pool<Sqlite>,
        movie_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "SELECT * FROM watchlist WHERE movie_id = ? LIMIT 1",
            movie_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(result.is_some())
    }
}
