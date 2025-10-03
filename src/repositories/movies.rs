use sqlx::{Pool, Sqlite};

use crate::{
    models::{movie::Movie, users::User},
    views::movie::WatchedMovieDetailed,
};

pub struct MovieRepository;

impl MovieRepository {
    pub async fn get_movie_by_id(
        pool: &Pool<Sqlite>,
        movie_id: i64,
    ) -> Result<Option<Movie>, sqlx::Error> {
        let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = ?")
            .bind(movie_id)
            .fetch_optional(pool)
            .await?;
        Ok(movie)
    }

    pub async fn find_watched_movies_by_username(
        user: &User,
        pool: Pool<Sqlite>,
    ) -> Result<Option<Vec<WatchedMovieDetailed>>, sqlx::Error> {
        let movies = sqlx::query_as::<_, WatchedMovieDetailed>(
        r#"
        SELECT wm.id, m.imdb_id, wm.user_id, wm.movie_id, m.title, m.genre, m.release_year as year, wm.watched_at, wm.rating
        FROM watched_movies wm
        INNER JOIN movies m ON wm.movie_id = m.id
        WHERE wm.user_id = ?
        ORDER BY wm.watched_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

        tracing::debug!(
            "Fetched watched movies for user {}: {:?}",
            user.username,
            movies
        );

        Ok(Some(movies))
    }
}
