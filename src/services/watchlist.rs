use crate::models::movie::Watchlist;
use crate::repositories::users::UserProfileRepository;
use crate::repositories::watchlist::WatchlistRepository;
use crate::{models::users::UserProfile, repositories::users::UserRepository};
use reqwest::StatusCode;
use sqlx::{Pool, Sqlite};

use crate::shared::error::Error;

pub struct WatchlistService;

impl WatchlistService {
    pub async fn get_user_watchlist(
        pool: &Pool<Sqlite>,
        username: &str,
    ) -> Result<(UserProfile, Vec<Watchlist>), Error> {
        let user = UserRepository::find_by_username(pool, username)
            .await
            .map_err(|_| Error::Status(StatusCode::INTERNAL_SERVER_ERROR))?
            .ok_or(Error::Status(StatusCode::NOT_FOUND))?;

        let profile = UserProfileRepository::from_user_id(pool, user.id).await?;

        let watchlist = WatchlistRepository::find_by_user_id(pool, user.id)
            .await
            .map_err(|_| Error::Status(StatusCode::INTERNAL_SERVER_ERROR))?;

        Ok((profile, watchlist))
    }

    pub async fn remove_from_watchlist(
        pool: &Pool<Sqlite>,
        user_id: i64,
        movie_id: i64,
    ) -> Result<(), Error> {
        WatchlistRepository::delete(pool, user_id, movie_id)
            .await
            .map_err(|_| Error::Status(StatusCode::INTERNAL_SERVER_ERROR))?;

        Ok(())
    }

    pub async fn delete_watchlisted_movie(
        movie_id: i64,
        user_id: i64,
        pool: Pool<Sqlite>,
    ) -> Result<(), Error> {
        let _ = sqlx::query!(
            "DELETE FROM watchlist WHERE user_id = ? AND movie_id = ?",
            user_id,
            movie_id
        )
        .execute(&pool)
        .await?;
        tracing::info!(
            "Deleted movie with ID {} from user {}'s watchlist",
            movie_id,
            user_id
        );
        Ok(())
    }

    pub async fn add_watchlsited_movie(
        user_id: i64,
        movie_id: i64,
        pool: Pool<Sqlite>,
    ) -> Result<(), Error> {
        let result = WatchlistRepository::add(&pool, user_id, movie_id).await;
        match result {
            Ok(_) => tracing::info!("Movie added to watchlist successfully"),
            Err(e) => match e.as_database_error() {
                Some(db_err) if db_err.code() == Some("2067".into()) => {
                    return Err(Error::Generic("Movie is already in your watchlist".into()));
                }
                _ => {
                    return Err(e.into());
                }
            },
        }
        Ok(())
    }
}
