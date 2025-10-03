use sqlx::{Pool, Sqlite};

use crate::models::movie::Watchlist;

pub struct WatchlistRepository;

impl WatchlistRepository {
    pub async fn find_by_user_id(
        pool: &Pool<Sqlite>,
        user_id: i64,
    ) -> Result<Vec<Watchlist>, sqlx::Error> {
        sqlx::query_as!(
            Watchlist,
            r#"SELECT id as "id!", user_id as "user_id!", movie_id as "movie_id!", added_at FROM watchlist WHERE user_id = ?"#,
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
}
