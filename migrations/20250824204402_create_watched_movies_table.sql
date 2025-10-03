-- Add migration script here

-- // Watched movies table
-- pub struct WatchedMovie {
--     pub id: i64,
--     pub user_id: i64,
--     pub movie_id: i64,
--     pub watched_at: DateTime<Utc>,
--     pub rating: Option<f32>, // User's rating if they provided one
-- }
--

CREATE TABLE IF NOT EXISTS watched_movies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    movie_id INTEGER NOT NULL,
    watched_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    rating REAL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (movie_id) REFERENCES movies(id) ON DELETE CASCADE,
    UNIQUE(user_id, movie_id)
);
