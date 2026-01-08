use backend::models::{
    imdb_stuff::{Genre, TmdbMovie},
    movie::{Movie, MovieState, WatchedMovie},
    users::User,
};
use sqlx::{Pool, Sqlite};
use time::OffsetDateTime;

pub async fn insert_watched_movie(
    pool: &Pool<Sqlite>,
    user_id: i64,
    movie_id: i64,
) -> WatchedMovie {
    sqlx::query_as::<_, WatchedMovie>(
        "INSERT INTO watched_movies (user_id, movie_id, watched_at, rating)
         VALUES (?, ?, ?, ?)
         RETURNING *",
    )
    .bind(user_id)
    .bind(movie_id)
    .bind(OffsetDateTime::now_utc())
    .bind(Some(8.0))
    .fetch_one(pool)
    .await
    .expect("Failed to insert test watched movie")
}

fn watched_movie() -> WatchedMovie {
    WatchedMovie {
        id: 1,
        user_id: 1,
        movie_id: 1,
        watched_at: OffsetDateTime::now_utc(),
        rating: Some(8.0),
    }
}

// CREATE TABLE IF NOT EXISTS movies (
//     id INTEGER PRIMARY KEY AUTOINCREMENT,
//     imdb_id VARCHAR(20) UNIQUE NOT NULL,
//     title VARCHAR(255) NOT NULL,
//     production_company VARCHAR(255) NOT NULL,
//     release_year INT NOT NULL,
//     genre VARCHAR(100) NOT NULL,
//     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//     updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
// );
pub async fn insert_movie(pool: &Pool<Sqlite>) -> Movie {
    let tmdb_movie = tmdb_movie();
    backend::repositories::movies::MovieRepository::insert_tmdb_movie(&tmdb_movie, pool)
        .await
        .expect("Failed to insert tmdb movie");

    let now = OffsetDateTime::now_utc();
    let mut movie = Movie {
        id: 0,
        imdb_id: "tt0111161".to_string(),
        title: "The Shawshank Redemption".to_string(),
        production_company: "some".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        state: MovieState::Available,
        created_at: Some(now),
        updated_at: Some(now),
    };

    let id = sqlx::query!(
        "INSERT INTO movies (imdb_id, title, production_company, release_year, genre, state, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        movie.imdb_id,
        movie.title,
        movie.production_company,
        movie.release_year,
        movie.genre,
        movie.state,
        movie.created_at,
        movie.updated_at
    )
    .execute(pool)
    .await
    .expect("Failed to insert test movie");
    movie.id = id.last_insert_rowid();
    movie
}

pub async fn insert_movie_custom(pool: &Pool<Sqlite>, tmdb_id: &str) -> Movie {
    // first insert into tmdb_movies to satisfy foreign key constraint
    let mut tmdb_movie = tmdb_movie();
    tmdb_movie.imdb_id = Some(tmdb_id.to_string());
    // generate a unique TMDB ID based on the imdb_id to avoid UNIQUE constraint violations
    // extract numeric part from imdb_id or use hash
    let numeric_id = tmdb_id
        .chars()
        .filter(|c| c.is_numeric())
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or_else(|_| {
            // fallback to hash if no numeric part
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            tmdb_id.hash(&mut hasher);
            (hasher.finish() as i64).abs()
        });
    tmdb_movie.id = numeric_id;
    backend::repositories::movies::MovieRepository::insert_tmdb_movie(&tmdb_movie, pool)
        .await
        .expect("Failed to insert tmdb movie");

    let now = OffsetDateTime::now_utc();
    let mut movie = Movie {
        id: 0,
        imdb_id: tmdb_id.to_string(),
        title: "The Shawshank Redemption".to_string(),
        production_company: "Frank Darabont".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        state: MovieState::Available,
        created_at: Some(now),
        updated_at: Some(now),
    };

    let id = sqlx::query!(
        "INSERT INTO movies (imdb_id, title, production_company, release_year, genre, state, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        movie.imdb_id,
        movie.title,
        movie.production_company,
        movie.release_year,
        movie.genre,
        movie.state,
        movie.created_at,
        movie.updated_at
    )
    .execute(pool)
    .await
    .expect("Failed to insert test movie");
    movie.id = id.last_insert_rowid();
    movie
}

pub fn tmdb_movie() -> TmdbMovie {
    TmdbMovie {
        id: 1,
        imdb_id: Some("tt0111161".to_string()),
        title: "The Shawshank Redemption".to_string(),
        overview: "Two imprisoned men".to_string(),
        release_date: "1994-09-23".to_string(),
        genres: vec![Genre {
            id: 18,
            name: "Drama".to_string(),
        }],
        poster_path: Some("/q6y0Go1tsGEsmtFryDOw3a9c.jpg".to_string()),
        backdrop_path: Some("/xBKGJQsAIeweesB79KC89qF7Z.jpg".to_string()),
        vote_average: 8.7,
        vote_count: 21000,
        popularity: 100.0,
        adult: false,
        original_language: "en".to_string(),
        original_title: "The Shawshank Redemption".to_string(),
        video: false,
        budget: 25000000,
        homepage: Some("https://www.warnerbros.com/movies/shawshank-redemption/".to_string()),
        origin_country: "US".to_string(),
        production_company: "Castle Rock Entertainment".to_string(),
        revenue: 28341469,
        runtime: Some(142),
        status: "Released".to_string(),
        tagline: Some("Fear can hold you prisoner. Hope can set you free.".to_string()),
    }
}

pub fn movie() -> Movie {
    Movie {
        id: 1,
        imdb_id: "tt0111161".to_string(),
        title: "The Shawshank Redemption".to_string(),
        production_company: "Castle Rock Entertainment".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        state: MovieState::Available,
        created_at: Some(OffsetDateTime::now_utc()),
        updated_at: Some(OffsetDateTime::now_utc()),
    }
}

pub async fn insert_user(pool: &Pool<Sqlite>, username: &str) -> User {
    use argon2::{
        Argon2, PasswordHasher,
        password_hash::{SaltString, rand_core::OsRng},
    };
    use chrono::DateTime;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password("hunter42".as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string();

    let some_date = DateTime::from_timestamp(1415923200, 0).expect("Failed to create test date");

    sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, display_name, is_admin, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(username)
    .bind(format!("{}@test.com", username))
    .bind(password_hash)
    .bind(None::<String>)
    .bind(false)
    .bind(some_date)
    .bind(some_date)
    .fetch_one(pool)
    .await
    .expect("Failed to insert test user")
}
