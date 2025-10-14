use sqlx::{Pool, Sqlite};

use crate::{
    clients::tmdb::TmdbClient,
    models::{
        imdb_stuff::{TmdbMovie, TmdbSearchResult},
        movie::Movie,
        users::User,
    },
    repositories::movies::MovieRepository,
    shared::{config::SETTINGS, error::Error},
    views::movie::WatchedMovieDetailed,
};

pub struct MovieService;

impl MovieService {
    pub async fn find_watched_movies_by_username(
        user: &User,
        pool: Pool<Sqlite>,
    ) -> Result<Option<Vec<WatchedMovieDetailed>>, Error> {
        let movies = MovieRepository::find_watched_movies_by_username(user, pool.clone()).await;

        tracing::debug!(
            "Fetched watched movies for user {}: {:?}",
            user.username,
            movies
        );

        let movies = match movies {
            Ok(movies) => movies,
            Err(e) => {
                tracing::error!("Database error: {}", e);
                return Err(Error::DatabaseError(e));
            }
        };

        Ok(movies)
    }

    pub async fn is_watchlisted(
        user_id: i64,
        movie_id: i64,
        pool: &Pool<Sqlite>,
    ) -> Result<bool, Error> {
        let result = sqlx::query!(
            "SELECT * FROM watchlist WHERE user_id = ? AND movie_id = ? LIMIT 1",
            user_id,
            movie_id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            Error::DatabaseError(e)
        })?;

        tracing::info!(user_id, movie_id, "Watchlist check",);

        Ok(result.is_some())
    }

    pub async fn is_watched(
        user_id: i64,
        movie_id: i64,
        pool: &Pool<Sqlite>,
    ) -> Result<bool, Error> {
        let result = sqlx::query!(
            "SELECT * FROM watched_movies WHERE user_id = ? AND movie_id = ? LIMIT 1",
            user_id,
            movie_id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            Error::DatabaseError(e)
        })?;

        tracing::info!(user_id, movie_id, "Watched check: {}", result.is_some());

        Ok(result.is_some())
    }

    pub async fn add_watched_movie(
        user_id: i64,
        movie_id: i64,
        rating: Option<i32>,
        pool: &Pool<Sqlite>,
    ) -> Result<(), Error> {
        let now = chrono::Utc::now();

        let _ = sqlx::query!(
            "INSERT INTO watched_movies (user_id, movie_id, watched_at, rating) VALUES (?,?,?,?)",
            user_id,
            movie_id,
            now,
            rating
        )
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            Error::DatabaseError(e)
        })?;

        tracing::info!(
            "Added movie with ID {} to user {}'s watched movies",
            movie_id,
            user_id
        );

        Ok(())
    }

    pub async fn search_tmdb_by_title(title: &str) -> Result<Vec<TmdbSearchResult>, Error> {
        tracing::info!("Searching TMDb for title: {}", title);
        let client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
        let movies = client.search_by_title(title).await?;
        Ok(movies)
    }

    pub async fn add_movie(tmdb_movie: &TmdbMovie, pool: &Pool<Sqlite>) -> Result<i64, Error> {
        let movie = Movie::from(tmdb_movie);
        let id = MovieRepository::add_movie(&movie, pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                Error::DatabaseError(e)
            })?;
        Ok(id)
    }

    pub async fn delete_movie(movie_id: i64, pool: &Pool<Sqlite>) -> Result<(), Error> {
        let _ = sqlx::query!("DELETE FROM movies WHERE id = ?", movie_id)
            .execute(pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                Error::DatabaseError(e)
            })?;
        tracing::info!("Deleted movie with ID {} from database", movie_id);
        Ok(())
    }
}
