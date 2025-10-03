-- Add migration script here

-- // Reviews table
-- pub struct Review {
--     pub id: i64,
--     pub user_id: i64,
--     pub movie_id: i64,
--     pub content: String,
--     pub rating: f32,
--     pub created_at: DateTime<Utc>,
--     pub updated_at: DateTime<Utc>,
-- }

CREATE TABLE IF NOT EXISTS reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    movie_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    rating REAL NOT NULL CHECK (rating >= 0 AND rating <= 10),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (movie_id) REFERENCES movies(id) ON DELETE CASCADE,
    UNIQUE(user_id, movie_id)
);
