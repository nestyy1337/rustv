-- Add migration script here
-- pub struct Watchlist {
--     pub id: i64,
--     pub user_id: i64,  // Foreign key to User
--     pub movie_id: i64, // Foreign key to Movie
--     pub added_at: DateTime<Utc>,
-- }

CREATE TABLE IF NOT EXISTS watchlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    movie_id INTEGER NOT NULL,
    added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (movie_id) REFERENCES movies(id) ON DELETE CASCADE,
    UNIQUE(user_id, movie_id)
);
