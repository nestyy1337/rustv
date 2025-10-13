use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TmdbMovie {
    pub adult: bool,
    pub backdrop_path: Option<String>,
    pub belongs_to_collection: Option<serde_json::Value>,
    pub budget: u64,
    pub genres: Vec<Genre>,
    pub homepage: Option<String>,
    pub id: i64,
    pub imdb_id: Option<String>,
    pub origin_country: Vec<String>,
    pub original_language: String,
    pub original_title: String,
    pub overview: String,
    pub popularity: f64,
    pub poster_path: Option<String>,
    pub production_companies: Vec<ProductionCompany>,
    pub production_countries: Vec<ProductionCountry>,
    pub release_date: String,
    pub revenue: u64,
    pub runtime: Option<i32>,
    pub spoken_languages: Vec<SpokenLanguage>,
    pub status: String,
    pub tagline: Option<String>,
    pub title: String,
    pub video: bool,
    pub vote_average: f64,
    pub vote_count: u32,
}

impl TmdbMovie {
    pub fn get_poster_url(&self) -> Option<String> {
        self.poster_path
            .as_ref()
            .map(|path| format!("https://image.tmdb.org/t/p/w500{}", path))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Genre {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductionCompany {
    pub id: i64,
    pub logo_path: Option<String>,
    pub name: String,
    pub origin_country: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductionCountry {
    pub iso_3166_1: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SpokenLanguage {
    pub english_name: String,
    pub iso_639_1: String,
    pub name: String,
}
