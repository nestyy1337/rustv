use sqlx::{Pool, Sqlite, types::Json};

use crate::{
    models::{
        imdb_stuff::TmdbMovie,
        movie::{Movie, MovieState},
        users::User,
    },
    shared::error::{DatabaseSnafu, Error, MovieMissingReason, MovieNotFoundSnafu, ResultExt},
    views::movie::WatchedMovieDetailed,
};

pub struct MovieRepository;

impl MovieRepository {
    pub async fn get_movie_by_id(
        movie_id: i64,
        pool: &Pool<Sqlite>,
    ) -> Result<Option<Movie>, Error> {
        let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE id = ?")
            .bind(movie_id)
            .fetch_optional(pool)
            .await
            .context(DatabaseSnafu {
                operation: "fetch movie by id",
            })?;
        tracing::debug!("Fetched movie: {:?}", movie);
        Ok(movie)
    }

    pub async fn find_watched_movies_by_username(
        user: &User,
        pool: Pool<Sqlite>,
    ) -> Result<Option<Vec<WatchedMovieDetailed>>, Error> {
        let movies = sqlx::query_as::<_, WatchedMovieDetailed>(
        r"
        SELECT wm.id, m.imdb_id, wm.user_id, wm.movie_id, m.title, m.genre, m.release_year as year, wm.watched_at, wm.rating
        FROM watched_movies wm
        INNER JOIN movies m ON wm.movie_id = m.id
        WHERE wm.user_id = ?
        ORDER BY wm.watched_at DESC
        ",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await.context(DatabaseSnafu {
        operation: "fetching watched movies by username",
    })?;

        tracing::debug!(
            user_id = ?user.id,
            movie_count = ?movies.len(),
            "Fetched watched movies for user"
        );

        Ok(Some(movies))
    }

    pub async fn find_rated_movies_by_username(
        user: &User,
        pool: Pool<Sqlite>,
    ) -> Result<Option<Vec<WatchedMovieDetailed>>, Error> {
        let movies = sqlx::query_as::<_, WatchedMovieDetailed>(
        r"
        SELECT wm.id, m.imdb_id, wm.user_id, wm.movie_id, m.title, m.genre, m.release_year as year, wm.watched_at, wm.rating
        FROM watched_movies wm
        INNER JOIN movies m ON wm.movie_id = m.id
        WHERE wm.user_id = ? AND wm.rating IS NOT NULL
        ORDER BY wm.watched_at DESC
        ",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await.context(DatabaseSnafu {
        operation: "fetching rated movies by username",
    })?;

        tracing::debug!(
            user_id = ?user.id,
            movie_count = ?movies.len(),
            "Fetched rated movies for user"
        );

        Ok(Some(movies))
    }

    pub async fn delete_watched_movie(
        user_id: i64,
        movie_id: i64,
        pool: &Pool<Sqlite>,
    ) -> Result<(), Error> {
        let _ = sqlx::query!(
            "DELETE FROM watched_movies WHERE user_id = ? AND movie_id = ?",
            user_id,
            movie_id
        )
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "deleting watched movie",
        })?;

        tracing::debug!(
            user_id = ?user_id,
            movie_id = ?movie_id,
            "Deleted watched movie"
        );
        Ok(())
    }

    pub async fn get_top10_latest_movies(pool: &Pool<Sqlite>) -> Result<Vec<Movie>, Error> {
        let movies = sqlx::query_as::<_, Movie>(
            r"
            SELECT * FROM movies
            WHERE state = ?
            ORDER BY release_year DESC
            LIMIT 10
            ",
        )
        .bind(MovieState::Available)
        .fetch_all(pool)
        .await
        .context(DatabaseSnafu {
            operation: "fetching top 10 latest available movies",
        })?;
        tracing::debug!(
            movie_count = ?movies.len(),
            "Fetched top 10 latest available movies"
        );
        Ok(movies)
    }

    pub async fn search_movie_by_title(
        pool: &Pool<Sqlite>,
        title: &str,
    ) -> Result<Vec<Movie>, Error> {
        let pattern = format!("%{title}%");
        let movies = sqlx::query_as::<_, Movie>(
            r"
            SELECT * FROM movies
            WHERE title LIKE ?
            ORDER BY release_year DESC
            ",
        )
        .bind(pattern)
        .fetch_all(pool)
        .await
        .context(DatabaseSnafu {
            operation: "searching movies by title",
        })?;
        tracing::debug!(
            search_title = %title,
            movie_count = ?movies.len(),
            "Searched movies by title"
        );

        Ok(movies)
    }

    pub async fn add_movie(movie: &Movie, pool: &Pool<Sqlite>) -> Result<i64, Error> {
        let id = sqlx::query!(
            r#"
            INSERT INTO movies (imdb_id, title, production_company, genre, release_year, state)
            VALUES (?, ?, ?, ?, ?, ?)"#,
            movie.imdb_id,
            movie.title,
            movie.production_company,
            movie.genre,
            movie.release_year,
            movie.state
        )
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "inserting new movie",
        })?;
        tracing::debug!(
            imdb_id = %movie.imdb_id,
            title = %movie.title,
            "Inserted new movie into database"
        );

        Ok(id.last_insert_rowid())
    }

    pub async fn get_movie_by_imdb_id(
        pool: &Pool<Sqlite>,
        imdb_id: &str,
    ) -> Result<Option<Movie>, Error> {
        let movie = sqlx::query_as::<_, Movie>("SELECT * FROM movies WHERE imdb_id = ?")
            .bind(imdb_id)
            .fetch_optional(pool)
            .await
            .context(DatabaseSnafu {
                operation: "fetching movie by imdb_id",
            })?;
        tracing::debug!(movie = ?movie, "Fetched movie by IMDb ID");

        Ok(movie)
    }

    pub async fn delete_movie_cascade(pool: &Pool<Sqlite>, movie_id: i64) -> Result<(), Error> {
        //TODO: we cannot return just the database error since the movie might just not exist
        // which is definitely not a 500 error.
        let _ = sqlx::query!("DELETE FROM movies WHERE id = ?", movie_id)
            .execute(pool)
            .await
            .context(DatabaseSnafu {
                operation: "deleting movie",
            })?;
        tracing::debug!(movie_id = ?movie_id, "Deleted movie and cascaded deletions");
        Ok(())
    }

    pub async fn find_requested_movies(pool: &Pool<Sqlite>) -> Result<Vec<Movie>, Error> {
        let movies = sqlx::query_as("SELECT * FROM movies WHERE state = ?")
            .bind(MovieState::Requested)
            .fetch_all(pool)
            .await
            .context(DatabaseSnafu {
                operation: "fetching requested movies",
            })?;
        tracing::debug!(
            movie_count = ?movies.len(),
            "Fetched requested movies"
        );
        Ok(movies)
    }

    pub async fn get_all_movies(pool: &Pool<Sqlite>) -> Result<Vec<Movie>, Error> {
        let movies = sqlx::query_as("SELECT * FROM movies")
            .fetch_all(pool)
            .await
            .context(DatabaseSnafu {
                operation: "fetching all movies",
            })?;
        tracing::debug!(
            movie_count = ?movies.len(),
            "Fetched all movies from database"
        );
        Ok(movies)
    }

    pub async fn get_all_available_movies(pool: &Pool<Sqlite>) -> Result<Vec<Movie>, Error> {
        let movies = sqlx::query_as!(
            Movie,
            "SELECT * FROM movies WHERE state = ?",
            MovieState::Available
        )
        .fetch_all(pool)
        .await
        .context(DatabaseSnafu {
            operation: "fetching all available movies",
        })?;
        tracing::debug!(
            movie_count = ?movies.len(),
            "Fetched all available movies from database"
        );
        Ok(movies)
    }

    pub async fn mark_available(movie_id: i64, pool: &Pool<Sqlite>) -> Result<(), Error> {
        let _ = sqlx::query!(
            "UPDATE movies SET state = ? WHERE id = ?",
            MovieState::Available,
            movie_id
        )
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "marking movie as available",
        })?;
        tracing::debug!(
            movie_id = ?movie_id,
            "Marked movie as available in database"
        );
        Ok(())
    }

    pub async fn insert_tmdb_movie(movie: &TmdbMovie, pool: &Pool<Sqlite>) -> Result<i64, Error> {
        let genres = Json(&movie.genres);

        let res = sqlx::query(
            r"
        INSERT OR IGNORE INTO tmdb_movies (
            adult, backdrop_path, budget, genres, homepage,
            id, imdb_id, origin_country, original_language,
            original_title, overview, popularity, poster_path,
            production_company, release_date, revenue, runtime,
            status, tagline, title, video, vote_average, vote_count
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23
        )
        ",
        )
        .bind(movie.adult)
        .bind(&movie.backdrop_path)
        .bind(movie.budget as i64)
        .bind(genres)
        .bind(&movie.homepage)
        .bind(movie.id)
        .bind(&movie.imdb_id)
        .bind(&movie.origin_country)
        .bind(&movie.original_language)
        .bind(&movie.original_title)
        .bind(&movie.overview)
        .bind(movie.popularity)
        .bind(&movie.poster_path)
        .bind(&movie.production_company)
        .bind(&movie.release_date)
        .bind(movie.revenue as i64)
        .bind(movie.runtime)
        .bind(&movie.status)
        .bind(&movie.tagline)
        .bind(&movie.title)
        .bind(movie.video)
        .bind(movie.vote_average)
        .bind(movie.vote_count as i32)
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "inserting TMDB movie",
        })?
        .last_insert_rowid();

        tracing::debug!(
            title = %movie.title,
            "Inserted TMDB movie into database"
        );

        Ok(res)
    }

    pub async fn get_tmdb_movie_by_imdb_id(
        imdb_id: &str,
        pool: &Pool<Sqlite>,
    ) -> Result<Option<TmdbMovie>, Error> {
        let movie = sqlx::query_as::<_, TmdbMovie>("SELECT * FROM tmdb_movies WHERE imdb_id = ?")
            .bind(imdb_id)
            .fetch_optional(pool)
            .await
            .context(DatabaseSnafu {
                operation: "fetching tmdb movie by imdb_id",
            })?;
        tracing::debug!(imdb_id = %imdb_id, movie = ?movie,
            "Fetched TMDB movie by IMDb ID"
        );
        Ok(movie)
    }

    pub async fn mark_requested(movie_id: i64, pool: &Pool<Sqlite>) -> Result<(), Error> {
        let result = sqlx::query!(
            "UPDATE movies set state = ? where id = ?",
            MovieState::Requested,
            movie_id
        )
        .execute(pool)
        .await
        .context(DatabaseSnafu {
            operation: "marking movie as requested",
        })?;
        if result.rows_affected() == 0 {
            tracing::warn!(
                movie_id = ?movie_id,
                "No movie found to mark as requested"
            );
            return Err(MovieNotFoundSnafu {
                movie_id,
                reason: MovieMissingReason::NoEntryDatabase,
            }
            .build())?;
        }
        tracing::debug!(
            movie_id = ?movie_id,
            "Marked movie as requested in database"
        );
        Ok(())
    }
}
