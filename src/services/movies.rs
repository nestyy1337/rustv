use snafu::ResultExt;
use sqlx::{Pool, Sqlite};

use crate::{
    clients::tmdb::TmdbClient,
    models::{
        imdb_stuff::TmdbSearchResult,
        movie::Watchlist,
        users::{User, UserProfile},
    },
    repositories::{
        movies::MovieRepository,
        users::{UserProfileRepository, UserRepository},
        watchlist::WatchlistRepository,
    },
    shared::{
        config::SETTINGS,
        error::{DatabaseSnafu, Error, SimpleUserNotFoundSnafu},
    },
    views::movie::WatchedMovieDetailed,
};

#[derive(Debug, Clone)]
pub struct SimpleMovieService {
    pool: Pool<Sqlite>,
}

impl SimpleMovieService {
    #[must_use]
    pub fn new(pool: Pool<Sqlite>) -> Self {
        SimpleMovieService { pool }
    }
}

#[async_trait::async_trait]
pub trait MovieService {
    async fn find_watched_movies_by_username(
        &self,
        user: &User,
    ) -> Result<Vec<WatchedMovieDetailed>, Error>;

    async fn is_watchlisted(&self, user_id: i64, movie_id: i64) -> Result<bool, Error>;

    async fn is_watched(&self, user_id: i64, movie_id: i64) -> Result<bool, Error>;

    async fn add_watched_movie(
        &self,
        user_id: i64,
        movie_id: i64,
        rating: Option<f32>,
    ) -> Result<(), Error>;

    async fn search_tmdb_by_title(&self, title: &str) -> Result<Vec<TmdbSearchResult>, Error>;

    async fn delete_movie(&self, movie_id: i64) -> Result<(), Error>;

    async fn get_user_watchlist(
        &self,
        username: &str,
    ) -> Result<(UserProfile, Vec<Watchlist>), Error>;

    async fn remove_from_watchlist(&self, user_id: i64, movie_id: i64) -> Result<(), Error>;

    async fn add_watchlisted_movie(&self, user_id: i64, movie_id: i64) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl MovieService for SimpleMovieService {
    #[tracing::instrument(name = "finding watched movies by username", skip(self))]
    async fn find_watched_movies_by_username(
        &self,
        user: &User,
    ) -> Result<Vec<WatchedMovieDetailed>, Error> {
        let movies = MovieRepository::find_watched_movies_by_username(user, self.pool.clone())
            .await?
            .unwrap_or_default();

        return Ok(movies);
    }

    #[tracing::instrument(name = "is movie watchlisted check", skip(self))]
    async fn is_watchlisted(&self, user_id: i64, movie_id: i64) -> Result<bool, Error> {
        let result = sqlx::query!(
            "SELECT * FROM watchlist WHERE user_id = ? AND movie_id = ? LIMIT 1",
            user_id,
            movie_id
        )
        .fetch_optional(&self.pool)
        .await
        .context(DatabaseSnafu {
            operation: "checking watchlist status",
        })?;

        tracing::info!("successfully checked watchlist status");

        Ok(result.is_some())
    }

    #[tracing::instrument(name = "is movie watched check", skip(self))]
    async fn is_watched(&self, user_id: i64, movie_id: i64) -> Result<bool, Error> {
        let result = sqlx::query!(
            "SELECT * FROM watched_movies WHERE user_id = ? AND movie_id = ? LIMIT 1",
            user_id,
            movie_id
        )
        .fetch_optional(&self.pool)
        .await
        .context(DatabaseSnafu {
            operation: "checking watched status",
        })?;

        tracing::info!(
            user_id = user_id,
            movie_id = movie_id,
            result = result.is_some(),
            "Watched check"
        );

        Ok(result.is_some())
    }

    #[tracing::instrument(name = "adding watched movie", skip(self))]
    async fn add_watched_movie(
        &self,
        user_id: i64,
        movie_id: i64,
        rating: Option<f32>,
    ) -> Result<(), Error> {
        let now = chrono::Utc::now();

        let _ = sqlx::query!(
            "INSERT INTO watched_movies (user_id, movie_id, watched_at, rating) VALUES (?,?,?,?) ON CONFLICT(user_id, movie_id) DO UPDATE SET watched_at=excluded.watched_at, rating=excluded.rating",
            user_id,
            movie_id,
            now,
            rating
        )
        .execute(&self.pool)
        .await.context(DatabaseSnafu {
                operation: "adding watched movie",
            })?;

        tracing::info!(
            movie_id = movie_id,
            user_id = user_id,
            rating = ?rating,
            "Added movie to user's watched movies"
        );

        Ok(())
    }

    #[tracing::instrument(name = "searching TMDb by title", skip(title))]
    async fn search_tmdb_by_title(&self, title: &str) -> Result<Vec<TmdbSearchResult>, Error> {
        tracing::info!(title = %title, "Searching TMDb for title");
        let client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
        let movies = client.search_by_title(title).await?;
        Ok(movies)
    }

    async fn delete_movie(&self, movie_id: i64) -> Result<(), Error> {
        MovieRepository::delete_movie_cascade(&self.pool, movie_id).await?;
        Ok(())
    }

    #[tracing::instrument(name = "getting user watchlist", skip(username))]
    async fn get_user_watchlist(
        &self,
        username: &str,
    ) -> Result<(UserProfile, Vec<Watchlist>), Error> {
        let user = UserRepository::find_by_username(&self.pool, username)
            .await?
            .ok_or_else(|| {
                SimpleUserNotFoundSnafu {
                    username: username.to_string(),
                }
                .build()
            })?;

        let profile = UserProfileRepository::from_user_id(&self.pool, user.id).await?;

        let watchlist = WatchlistRepository::find_by_user_id(&self.pool, user.id).await?;

        Ok((profile, watchlist))
    }

    #[tracing::instrument(name = "removing movie from watchlist", skip(self))]
    async fn remove_from_watchlist(&self, user_id: i64, movie_id: i64) -> Result<(), Error> {
        WatchlistRepository::delete(&self.pool, user_id, movie_id).await?;

        Ok(())
    }

    #[tracing::instrument(name = "adding movie to watchlist", skip(self))]
    async fn add_watchlisted_movie(&self, user_id: i64, movie_id: i64) -> Result<(), Error> {
        let res = WatchlistRepository::add(&self.pool, user_id, movie_id).await?;
        dbg!(&res);
        Ok(())
    }
}
