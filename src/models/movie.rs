use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Serialize, FromRow)]
pub struct Watchlist {
    pub id: i64,
    pub user_id: i64,
    pub movie_id: i64,
    pub added_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Movie {
    pub id: i64,
    pub title: String,
    pub director: String,
    pub release_year: i64,
    pub genre: String,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct WatchedMovie {
    pub id: i64,
    pub user_id: i64,
    pub movie_id: i64,
    pub watched_at: time::OffsetDateTime,
    pub rating: Option<f32>,
}

pub struct Review {
    pub id: i64,
    pub user_id: i64,
    pub movie_id: i64,
    pub content: String,
    pub rating: f32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
