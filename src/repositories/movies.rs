use sqlx::{Pool, Sqlite};

use crate::{
    models::{movie::Movie, users::User},
    shared::error::Error,
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

    pub async fn delete_watched_movie(
        user_id: i64,
        movie_id: i64,
        pool: &Pool<Sqlite>,
    ) -> Result<(), Error> {
        tracing::info!(
            "Deleting movie with ID {} from user {}'s watched movies",
            movie_id,
            user_id
        );
        let _ = sqlx::query!(
            "DELETE FROM watched_movies WHERE user_id = ? AND movie_id = ?",
            user_id,
            movie_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_top10_latest_movies(pool: &Pool<Sqlite>) -> Result<Vec<Movie>, sqlx::Error> {
        let movies = sqlx::query_as::<_, Movie>(
            r#"
            SELECT * FROM movies
            ORDER BY release_year DESC
            LIMIT 10
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(movies)
    }

    pub async fn search_movie_by_title(
        pool: &Pool<Sqlite>,
        title: &str,
    ) -> Result<Vec<Movie>, sqlx::Error> {
        let pattern = format!("%{}%", title);
        let movies = sqlx::query_as::<_, Movie>(
            r#"
            SELECT * FROM movies
            WHERE title LIKE ?
            ORDER BY release_year DESC
            "#,
        )
        .bind(pattern)
        .fetch_all(pool)
        .await?;

        Ok(movies)
    }

    pub async fn add_movie(movie: &Movie, pool: &Pool<Sqlite>) -> Result<i64, sqlx::Error> {
        let id = sqlx::query!(
            r#"
            INSERT INTO movies (imdb_id, title,director, genre, release_year, available)
            VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(imdb_id) DO NOTHING"#,
            movie.imdb_id,
            movie.title,
            movie.director,
            movie.genre,
            movie.release_year,
            movie.available
        )
        .execute(pool)
        .await?;

        Ok(id.last_insert_rowid())
    }

    pub async fn get_movie_by_imdb_id(
        pool: &Pool<Sqlite>,
        imdb_id: &str,
    ) -> Result<Option<Movie>, sqlx::Error> {
        let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE imdb_id = ?")
            .bind(imdb_id)
            .fetch_optional(pool)
            .await?;
        Ok(movie)
    }

    pub async fn delete_movie_cascade(
        pool: &Pool<Sqlite>,
        movie_id: i64,
    ) -> Result<(), sqlx::Error> {
        let _ = sqlx::query!("DELETE FROM movies WHERE id = ?", movie_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
