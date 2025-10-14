use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::models::imdb_stuff::TmdbMovie;

#[derive(Debug, Serialize, FromRow)]
pub struct Watchlist {
    pub id: i64,
    pub user_id: i64,
    pub movie_id: i64,
    pub available: bool,
    pub added_at: time::OffsetDateTime,
}

impl Watchlist {
    pub fn is_available(&self) -> bool {
        self.available
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Movie {
    pub id: i64,
    pub imdb_id: String,
    pub title: String,
    pub director: String,
    pub release_year: i64,
    pub genre: String,
    pub available: bool,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

impl Movie {
    pub fn created_at_string(&self) -> String {
        self.created_at
            .map(|d| {
                d.format(&time::format_description::well_known::Iso8601::DEFAULT)
                    .unwrap_or_default()
                    .to_string()
            })
            .unwrap_or_default()
    }
}

impl From<TmdbMovie> for Movie {
    fn from(value: TmdbMovie) -> Self {
        Movie {
            id: 0,
            imdb_id: value.imdb_id.unwrap_or_default(),
            title: value.title,
            director: "Unknown".to_string(),
            release_year: 0,
            genre: value
                .genres
                .into_iter()
                .next()
                .unwrap_or_default()
                .to_string(),
            available: false,
            created_at: None,
            updated_at: None,
        }
    }
}

impl From<&TmdbMovie> for Movie {
    fn from(value: &TmdbMovie) -> Self {
        Movie {
            id: 0,
            imdb_id: value.imdb_id.clone().unwrap_or_default(),
            title: value.title.clone(),
            director: "Unknown".to_string(),
            release_year: 0,
            genre: value
                .genres
                .clone()
                .into_iter()
                .next()
                .unwrap_or_default()
                .to_string(),
            available: false,
            created_at: None,
            updated_at: None,
        }
    }
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
