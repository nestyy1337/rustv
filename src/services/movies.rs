use sqlx::{Pool, Sqlite};

use crate::{
    models::users::User,
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

    #[allow(dead_code)]
    async fn get_imdb_id_internal(
        movie_id: i64,
        pool: Pool<Sqlite>,
    ) -> Result<Option<String>, Error> {
        let mapping = sqlx::query_scalar::<_, String>(
            "SELECT imdb_id FROM imdb_mapping WHERE id = ? LIMIT 1",
        )
        .bind(movie_id)
        .fetch_optional(&pool)
        .await?;
        Ok(mapping)
    }

    #[allow(dead_code)]
    async fn get_imdb_id_api(movie_title: &str) -> Result<Option<String>, Error> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!(
                "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
                SETTINGS.application.apikeys.tmdb, movie_title
            ))
            .send()
            .await;
        let body = match resp {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("Failed to fetch movie data from TMDB: {}", e);
                return Err(Error::ReqwestError(e));
            }
        };
        Ok(Some(body.text().await.unwrap()))
    }
}
