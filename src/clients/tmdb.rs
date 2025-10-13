use axum::body::Bytes;

use crate::{models::imdb_stuff::TmdbMovie, shared::error::Error};

pub struct TmdbClient {
    pub api_key: String,
    pub base_url: String,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
        }
    }

    pub async fn get_movie_details(&self, movie_id: &str) -> Result<TmdbMovie, Error> {
        let url = format!("{}/movie/{}", self.base_url, movie_id);
        tracing::info!("Fetching movie details from URL: {}", url);
        let client = reqwest::Client::new();
        let res = client
            .get(&url)
            .query(&[("api_key", &self.api_key)])
            .send()
            .await?;
        let text = res.text().await?;
        println!("Response text: {}", text);
        let res = serde_json::from_str::<TmdbMovie>(&text)?;
        tracing::info!("Fetched movie details for ID {}: {:?}", movie_id, res);

        Ok(res)
    }

    pub async fn get_poster(&self, movie: TmdbMovie, movie_id: i64) -> Result<Bytes, Error> {
        if let Some(poster_path) = movie.poster_path {
            let poster_url = format!("https://image.tmdb.org/t/p/w500{}", poster_path);
            tracing::info!("Poster URL for movie {}: {}", movie.title, poster_url);
            let client = reqwest::Client::new();
            let res = client.get(&poster_url).send().await?;
            if res.status().is_success() {
                tracing::info!("Successfully fetched poster for movie {}", movie.title);
                let body = res.bytes().await?;
                let file_path = format!("movies/{}_poster.jpg", movie_id);
                tokio::fs::File::create(&file_path).await?;
                tokio::fs::write(&file_path, &body).await?;
                tracing::info!("Saved poster to {}", file_path);
                return Ok(body);
            } else {
                tracing::error!(
                    "Failed to fetch poster for movie {}: HTTP {}",
                    movie.title,
                    res.status()
                );
            }
        } else {
            tracing::info!("No poster available for movie {}", movie.title);
        }
        Err(Error::MovieNotFound)
    }
}
