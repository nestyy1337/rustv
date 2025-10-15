use backend::models::{
    imdb_stuff::{Genre, TmdbMovie},
    movie::{Movie, WatchedMovie},
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
//     director VARCHAR(255) NOT NULL,
//     release_year INT NOT NULL,
//     genre VARCHAR(100) NOT NULL,
//     created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//     updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
// );
pub async fn insert_movie(pool: &Pool<Sqlite>) -> Movie {
    let now = OffsetDateTime::now_utc();
    let mut movie = Movie {
        id: 0,
        imdb_id: "tt0111161".to_string(),
        title: "The Shawshank Redemption".to_string(),
        director: "Frank Darabont".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        available: true,
        created_at: Some(now),
        updated_at: Some(now),
    };

    let id = sqlx::query!(
        "INSERT INTO movies (imdb_id, title, director, release_year, genre, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        movie.imdb_id,
        movie.title,
        movie.director,
        movie.release_year,
        movie.genre,
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
    let now = OffsetDateTime::now_utc();
    let mut movie = Movie {
        id: 0,
        imdb_id: tmdb_id.to_string(),
        title: "The Shawshank Redemption".to_string(),
        director: "Frank Darabont".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        available: true,
        created_at: Some(now),
        updated_at: Some(now),
    };

    let id = sqlx::query!(
        "INSERT INTO movies (imdb_id, title, director, release_year, genre, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        movie.imdb_id,
        movie.title,
        movie.director,
        movie.release_year,
        movie.genre,
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
        release_date: "1994-0923".to_string(),
        genres: vec![Genre {
            id: 18,
            name: "Drama".to_string(),
        }],
        poster_path: Some(
            "/q6y0Go1tsGEsmtFryDO
w3a9c.jpg"
                .to_string(),
        ),
        backdrop_path: Some("/xBKGJQsAIeweesB79KC89qF7Z.jpg".to_string()),
        vote_average: 8.7,
        vote_count: 21000,
        popularity: 100.0,
        adult: false,
        original_language: "en".to_string(),
        original_title: "The Shawshank Redemption".to_string(),
        video: false,
        belongs_to_collection: None,
        budget: 25000000,
        homepage: Some("https://www.warnerbros.com/movies/shawshank-redemption/".to_string()),
        origin_country: vec!["US".to_string()],
        production_companies: vec![],
        production_countries: vec![],
        revenue: 28341469,
        runtime: Some(142),
        spoken_languages: vec![],
        status: "Released".to_string(),
        tagline: Some("Fear can hold you prisoner. Hope can set you free.".to_string()),
    }
}

pub fn movie() -> Movie {
    Movie {
        id: 1,
        imdb_id: "tt0111161".to_string(),
        title: "The Shawshank Redemption".to_string(),
        director: "Frank Darabont".to_string(),
        release_year: 1994,
        genre: "Drama".to_string(),
        available: true,
        created_at: Some(OffsetDateTime::now_utc()),
        updated_at: Some(OffsetDateTime::now_utc()),
    }
}
