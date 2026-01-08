use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

fn first_element_vec<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Vec<String> = Vec::deserialize(deserializer)?;
    Ok(v.into_iter().next().unwrap_or_default())
}

fn first_element_map_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Vec<ProductionCompany> = Vec::deserialize(deserializer)?;
    Ok(v.into_iter().next().map(|pc| pc.name).unwrap_or_default())
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TmdbMovie {
    pub adult: bool,
    pub backdrop_path: Option<String>,
    pub budget: u64,
    #[sqlx(json)]
    pub genres: Vec<Genre>,
    pub homepage: Option<String>,
    pub id: i64,
    pub imdb_id: Option<String>,
    #[serde(deserialize_with = "first_element_vec")]
    pub origin_country: String,
    pub original_language: String,
    pub original_title: String,
    pub overview: String,
    pub popularity: f64,
    pub poster_path: Option<String>,
    #[serde(
        rename = "production_companies",
        deserialize_with = "first_element_map_name"
    )]
    pub production_company: String,
    pub release_date: String,
    pub revenue: u64,
    pub runtime: Option<i32>,
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

#[derive(Debug, Deserialize, Serialize, Clone, FromRow)]
pub struct Genre {
    pub id: i32,
    pub name: String,
}

impl Default for Genre {
    fn default() -> Self {
        Genre {
            id: 0,
            name: "Unknown".to_string(),
        }
    }
}

impl std::fmt::Display for Genre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Default, Deserialize, Serialize, FromRow)]
pub struct ProductionCompany {
    pub id: i64,
    pub logo_path: Option<String>,
    pub name: String,
    pub origin_country: String,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct ProductionCountry {
    pub iso_3166_1: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct SpokenLanguage {
    pub english_name: String,
    pub iso_639_1: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TmdbSearchResult {
    pub id: i32,
    pub title: String,
    pub original_title: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub popularity: f64,
    pub vote_average: f64,
    pub vote_count: i32,
    pub adult: bool,
    pub genre_ids: Vec<i32>,
    pub video: bool,
}
