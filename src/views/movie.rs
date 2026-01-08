use askama::Template;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use time::OffsetDateTime;

use crate::{
    models::{imdb_stuff::TmdbMovie, movie::Movie, users::UserProfile},
    services::movie_manager::StreamableVideo,
};

#[derive(Template)]
#[template(path = "player.html")]
pub struct MoviePlayerData<'a> {
    pub timestamp: usize,
    pub movie: &'a Movie,
    pub csrf_token: String,
}

#[derive(Template)]
#[template(path = "watched_movies.html")]
pub struct WatchedMovieData {
    pub profile: UserProfile,
    pub movies: Vec<WatchedMovieDetailed>,
    pub csrf_token: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
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
    pub fn new(
        movies: Vec<WatchedMovieDetailed>,
        profile: UserProfile,
        csrf_token: String,
    ) -> Self {
        Self {
            profile,
            movies,
            csrf_token,
        }
    }
}

pub fn format_imdb_url(imdb_id: &str) -> String {
    format!("https://www.imdb.com/title/{}/", imdb_id)
}

#[derive(Template)]
#[template(path = "ratings.html")]
pub struct RatingsPageData {
    pub profile: UserProfile,
    pub movies: Vec<WatchedMovieDetailed>,
    pub csrf_token: String,
}

impl RatingsPageData {
    pub fn new(
        movies: Vec<WatchedMovieDetailed>,
        profile: UserProfile,
        csrf_token: String,
    ) -> Self {
        Self {
            profile,
            movies,
            csrf_token,
        }
    }
}

#[derive(Template)]
#[template(path = "movie_details.html")]
pub struct MovieDetailsData {
    pub movie: Movie,
    pub tmdb_movie: TmdbMovie,
    pub streamable: Option<StreamableVideo>,
    pub watchlisted: bool,
    pub watched: bool,
    pub profile: UserProfile,
    pub csrf_token: String,
}

impl MovieDetailsData {
    pub fn new(
        movie: Movie,
        tmdb_movie: TmdbMovie,
        streamble: Option<StreamableVideo>,
        watchlisted: bool,
        watched: bool,
        user_profile: UserProfile,
        csrf_token: String,
    ) -> Self {
        Self {
            movie,
            tmdb_movie,
            streamable: streamble,
            watchlisted,
            watched,
            profile: user_profile,
            csrf_token,
        }
    }
}

#[derive(Template)]
#[template(path = "steal_movies.html")]
pub struct StealMoviesData {
    pub csrf_token: String,
}

#[derive(Template)]
#[template(path = "requested_movies.html")]
pub struct RequestedMoviesTemplate {
    pub movies: Vec<Movie>,
    pub csrf_token: String,
}
