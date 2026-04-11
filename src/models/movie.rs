use std::path::PathBuf;

use time::macros::format_description;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use time::format_description::BorrowedFormatItem;

use crate::models::imdb_stuff::TmdbMovie;

#[derive(Debug, Serialize, FromRow)]
pub struct Watchlist {
    pub id: i64,
    pub user_id: i64,
    pub movie_id: i64,
    pub state: MovieState,
    pub added_at: time::OffsetDateTime,
}

impl Watchlist {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.state.is_available()
    }
}

#[derive(Debug, Clone, Copy, sqlx::Type, PartialEq, Serialize, Deserialize, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum MovieState {
    #[default]
    Unavailable = 0,
    Available = 1,
    Processing = 2,
    Requested = 3,
    NotRequested = 4,
    Downloading = 5,
    Downloaded = 6,
}

impl From<i64> for MovieState {
    fn from(val: i64) -> Self {
        match val {
            0 => MovieState::Unavailable,
            1 => MovieState::Available,
            2 => MovieState::Processing,
            3 => MovieState::Requested,
            4 => MovieState::NotRequested,
            5 => MovieState::Downloading,
            6 => MovieState::Downloaded,
            _ => MovieState::Unavailable,
        }
    }
}

impl std::fmt::Display for MovieState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            MovieState::Unavailable => "Unavailable",
            MovieState::Available => "Available",
            MovieState::Processing => "Processing",
            MovieState::Requested => "Requested",
            MovieState::NotRequested => "Not Requested",
            MovieState::Downloading => "Downloading",
            MovieState::Downloaded => "Downloaded",
        };
        write!(f, "{state_str}")
    }
}
impl MovieState {
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, MovieState::Available)
    }
    #[must_use]
    pub fn is_processing(&self) -> bool {
        matches!(self, MovieState::Processing)
    }
    #[must_use]
    pub fn is_unavailable(&self) -> bool {
        matches!(self, MovieState::Unavailable)
    }
    #[must_use]
    pub fn is_requested(&self) -> bool {
        matches!(self, MovieState::Requested)
    }
    #[must_use]
    pub fn is_downloading(&self) -> bool {
        matches!(self, MovieState::Downloading)
    }
    #[must_use]
    pub fn is_downloaded(&self) -> bool {
        matches!(self, MovieState::Downloaded)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MoviePath {
    Local(PathBuf),
    Remote(String),
}

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Eq, Clone)]
pub struct Movie {
    pub id: i64,
    pub imdb_id: String,
    pub title: String,
    pub production_company: String,
    pub release_year: i64,
    pub genre: String,
    pub state: MovieState,
    pub created_at: Option<time::OffsetDateTime>,
    pub updated_at: Option<time::OffsetDateTime>,
}

impl AsRef<Movie> for Movie {
    fn as_ref(&self) -> &Movie {
        self
    }
}

const DATEFORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[hour]:[minute] [month repr:short] [day], [year]");

impl Movie {
    #[must_use]
    pub fn created_at_string(&self) -> String {
        self.created_at
            .map(|d| d.format(&DATEFORMAT).unwrap_or_default().clone())
            .unwrap_or_default()
    }
}

impl From<TmdbMovie> for Movie {
    fn from(value: TmdbMovie) -> Self {
        Movie {
            id: 0,
            imdb_id: value.imdb_id.unwrap_or_default(),
            title: value.title,
            production_company: value.production_company,
            release_year: value
                .release_date
                .split_once('-')
                .map_or(0, |(y, _)| y.parse::<i64>().unwrap_or(0)),
            genre: value
                .genres
                .into_iter()
                .next()
                .unwrap_or_default()
                .to_string(),
            state: MovieState::Unavailable,
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
            production_company: value.production_company.clone(),
            release_year: value
                .release_date
                .split_once('-')
                .map_or(0, |(y, _)| y.parse::<i64>().unwrap_or(0)),
            genre: value
                .genres
                .first()
                .map(std::string::ToString::to_string)
                .unwrap_or_default(),
            state: MovieState::Unavailable,
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
