use askama::Template;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use time::OffsetDateTime;

use crate::models::users::UserProfile;

#[derive(Template)]
#[template(path = "watched_movies.html")]
pub struct WatchedMovieData {
    pub profile: UserProfile,
    pub movies: Vec<WatchedMovieDetailed>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct WatchedMovieDetailed {
    pub id: i64,
    pub imdb_id: Option<String>,
    pub user_id: i64,
    pub movie_id: i64,
    pub title: String,
    pub genre: String,
    pub year: i64,
    pub watched_at: OffsetDateTime,
    pub rating: Option<f32>,
}

impl WatchedMovieDetailed {
    pub fn format_date(&self) -> String {
        let format = time::format_description::parse("[month repr:short] [day], [year]").unwrap();
        self.watched_at
            .format(&format)
            .unwrap_or_else(|_| "Unknown date".to_string())
    }

    pub fn imdb_url(&self) -> String {
        if let Some(id) = &self.imdb_id {
            format_imdb_url(id)
        } else {
            "NOT AVAILABLE".to_string()
        }
    }
}

impl WatchedMovieData {
    pub fn new(movies: Vec<WatchedMovieDetailed>, profile: UserProfile) -> Self {
        Self { profile, movies }
    }
}

pub fn format_imdb_url(imdb_id: &str) -> String {
    format!("https://www.imdb.com/title/{}/", imdb_id)
}

#[derive(Template)]
#[template(path = "ratings.html")]
pub struct RatingsPageData {
    pub profile: UserProfile,
}

impl RatingsPageData {
    pub fn new(profile: UserProfile) -> Self {
        Self { profile }
    }
}
