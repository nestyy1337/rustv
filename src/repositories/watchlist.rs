use snafu::ResultExt;
use sqlx::{Pool, Sqlite};

use crate::{
    models::movie::Watchlist,
    shared::error::{
        DatabaseSnafu, Error, MovieAlreadyWatchlistedSnafu, WatchlistedMovieNotFoundSnafu,
    },
};

pub struct WatchlistRepository;

impl WatchlistRepository {
    pub async fn find_by_user_id(
        pool: &Pool<Sqlite>,
        user_id: i64,
    ) -> Result<Vec<Watchlist>, Error> {
        sqlx::query_as!(
            Watchlist,
            r#"SELECT w.id as "id!", w.user_id as "user_id!", w.movie_id as "movie_id!", m.state as state, w.added_at FROM watchlist w
            INNER JOIN movies m ON w.movie_id = m.id
            WHERE user_id = ?"#,
            user_id
        )
        .fetch_all(pool)
        .await
        .context(DatabaseSnafu {
            operation: "finding watchlist by user id",
        })
    }

    pub async fn delete(pool: &Pool<Sqlite>, user_id: i64, movie_id: i64) -> Result<u64, Error> {
        let result = sqlx::query!(
            "DELETE FROM watchlist WHERE user_id = ? AND movie_id = ?",
            user_id,
            movie_id
        )
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "deleting from watchlist",
        })?;

        Ok(result.rows_affected())
    }

    pub async fn add(pool: &Pool<Sqlite>, user_id: i64, movie_id: i64) -> Result<(), Error> {
        let now = chrono::Utc::now();

        let res = sqlx::query!(
            "INSERT INTO watchlist (user_id, movie_id, added_at) VALUES (?,?,?)",
            user_id,
            movie_id,
            now
        )
        .execute(pool)
        .await;

        if let Err(e) = &res {
            if let sqlx::Error::Database(db_err) = e {
                match db_err.code().as_deref() {
                    Some("2067") => {
                        return Err(MovieAlreadyWatchlistedSnafu { movie_id, user_id }.build())?;
                    }
                    Some("787") => {
                        return Err(WatchlistedMovieNotFoundSnafu { movie_id, user_id }.build())?;
                    }
                    _ => {}
                }
            }
            res.context(DatabaseSnafu {
                operation: "adding to watchlist",
            })?;
        }
        Ok(())
    }

    pub async fn is_watchlisted_anywhere(
        pool: &Pool<Sqlite>,
        movie_id: i64,
    ) -> Result<bool, Error> {
        let result = sqlx::query!(
            "SELECT * FROM watchlist WHERE movie_id = ? LIMIT 1",
            movie_id
        )
        .fetch_optional(pool)
        .await
        .context(DatabaseSnafu {
            operation: "checking if movie is watchlisted",
        })?;

        Ok(result.is_some())
    }
}
