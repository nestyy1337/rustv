use axum::body::Bytes;
use snafu::ResultExt;

use crate::{
    models::imdb_stuff::{TmdbMovie, TmdbSearchResult},
    shared::error::{
        ClientRequestSnafu, DeserializationFailedSnafu, Error, MissingFieldSnafu, TokioIoSnafu,
    },
};

#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    async fn get_text(&self, url: &str, query_params: Vec<(&str, &str)>) -> Result<String, Error>;
    async fn get_bytes(&self, url: &str) -> Result<(Bytes, u16), Error>;
}

#[derive(Clone)]
pub struct ReqwestHttpClient;

#[async_trait::async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get_text(&self, url: &str, query_params: Vec<(&str, &str)>) -> Result<String, Error> {
        let client = reqwest::Client::new();
        let res =
            client
                .get(url)
                .query(&query_params)
                .send()
                .await
                .context(ClientRequestSnafu {
                    operation: "making HTTP GET request",
                    client: "TMDB",
                    url: Some(url.to_string()),
                })?;

        res.text().await.context(ClientRequestSnafu {
            operation: "reading response text",
            client: "TMDB",
            url: Some(url.to_string()),
        })
    }

    async fn get_bytes(&self, url: &str) -> Result<(Bytes, u16), Error> {
        let client = reqwest::Client::new();
        let res = client.get(url).send().await.context(ClientRequestSnafu {
            operation: "making HTTP GET request for bytes",
            client: "TMDB",
            url: Some(url.to_string()),
        })?;

        let status = res.status().as_u16();
        let bytes = res.bytes().await.context(ClientRequestSnafu {
            operation: "reading response bytes",
            client: "TMDB",
            url: Some(url.to_string()),
        })?;

        Ok((bytes, status))
    }
}

pub struct TmdbClient<T: HttpClient> {
    pub api_key: String,
    pub base_url: String,
    http_client: T,
}

impl TmdbClient<ReqwestHttpClient> {
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
            http_client: ReqwestHttpClient,
        }
    }
}

impl<T: HttpClient> TmdbClient<T> {
    pub fn with_client(api_key: String, http_client: T) -> Self {
        Self {
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
            http_client,
        }
    }

    pub async fn get_movie_details(&self, movie_id: &str) -> Result<TmdbMovie, Error> {
        let url = format!("{}/movie/{}", self.base_url, movie_id);
        tracing::info!(url = %url, "Fetching movie details from URL");

        let text = self
            .http_client
            .get_text(&url, vec![("api_key", &self.api_key)])
            .await?;

        let res = serde_json::from_str::<TmdbMovie>(&text);
        let res = res.context(DeserializationFailedSnafu {})?;
        tracing::info!(movie_id = %movie_id, movie = ?res, "Fetched movie details");

        Ok(res)
    }

    pub async fn get_poster(&self, movie: TmdbMovie, movie_id: i64) -> Result<Bytes, Error> {
        if let Some(poster_path) = movie.poster_path {
            let poster_url = format!("https://image.tmdb.org/t/p/w500{poster_path}");
            tracing::info!(movie_title = %movie.title, poster_url = %poster_url, "Poster URL for movie");

            let (body, status) = self.http_client.get_bytes(&poster_url).await?;

            if status == 200 {
                tracing::info!(movie_title = %movie.title, "Successfully fetched poster for movie");
                let file_path = format!("movies/{movie_id}/poster.jpg");
                if let Some(parent) = std::path::Path::new(&file_path).parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .context(TokioIoSnafu {
                            operation: "creating directories for poster",
                        })?;
                }
                tokio::fs::File::create(&file_path)
                    .await
                    .context(TokioIoSnafu {
                        operation: "creating poster file",
                    })?;

                tokio::fs::write(&file_path, &body)
                    .await
                    .context(TokioIoSnafu {
                        operation: "writing poster file",
                    })?;
                tracing::info!(file_path = %file_path, "Saved poster");
                return Ok(body);
            }
            tracing::error!(
                movie_title = %movie.title,
                status = status,
                "Failed to fetch poster for movie"
            );
        } else {
            tracing::info!(movie_title = %movie.title, "No poster available for movie");
        }
        Err(MissingFieldSnafu {
            field: "poster_path",
        }
        .build())?
    }

    pub async fn search_by_title(&self, title: &str) -> Result<Vec<TmdbSearchResult>, Error> {
        let url = format!("{}/search/movie", self.base_url);
        tracing::info!(title = %title, "Searching for movies with title");

        let title_string = title.to_string();
        let text = self
            .http_client
            .get_text(
                &url,
                vec![("api_key", &self.api_key), ("query", &title_string)],
            )
            .await?;

        let search_result: serde_json::Value =
            serde_json::from_str(&text).context(DeserializationFailedSnafu {})?;
        let movies = if let Some(results) = search_result.get("results") {
            serde_json::from_value::<Vec<TmdbSearchResult>>(results.clone())
                .context(DeserializationFailedSnafu {})?
        } else {
            vec![]
        };
        tracing::info!(count = movies.len(), title = %title, "Found movies matching title");
        Ok(movies)
    }

    pub async fn search_by_imdb_id(
        &self,
        imdb_id: &str,
    ) -> Result<Option<TmdbSearchResult>, Error> {
        let url = format!("{}/search/movie", self.base_url);
        tracing::info!(imdb_id = %imdb_id, "Searching for movies with id");

        let imdb_id_string = imdb_id.to_string();
        let text = self
            .http_client
            .get_text(
                &url,
                vec![("api_key", &self.api_key), ("query", &imdb_id_string)],
            )
            .await?;

        let search_result: serde_json::Value =
            serde_json::from_str(&text).context(DeserializationFailedSnafu {})?;
        let movies = if let Some(results) = search_result.get("results") {
            serde_json::from_value::<Vec<TmdbSearchResult>>(results.clone())
                .context(DeserializationFailedSnafu {})?
                .into_iter()
                .next()
        } else {
            None
        };
        tracing::info!(imdb_id = ?imdb_id, "found movie");
        Ok(movies)
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::error::GenericSnafu;

    use super::*;

    struct MockHttpClient {
        responses: std::collections::HashMap<String, String>,
        byte_responses: std::collections::HashMap<String, (Vec<u8>, u16)>,
    }

    impl MockHttpClient {
        fn new() -> Self {
            Self {
                responses: std::collections::HashMap::new(),
                byte_responses: std::collections::HashMap::new(),
            }
        }

        fn set_response(&mut self, url: String, response: String) {
            self.responses.insert(url, response);
        }

        fn set_byte_response(&mut self, url: String, data: Vec<u8>, status: u16) {
            self.byte_responses.insert(url, (data, status));
        }
    }

    #[async_trait::async_trait]
    impl HttpClient for MockHttpClient {
        async fn get_text(
            &self,
            url: &str,
            _query_params: Vec<(&str, &str)>,
        ) -> Result<String, Error> {
            if let Some(response) = self.responses.get(url) {
                Ok(response.clone())
            } else {
                Err(GenericSnafu {
                    reason: format!("No mock response for URL: {}", url),
                }
                .build())
            }
        }

        async fn get_bytes(&self, url: &str) -> Result<(Bytes, u16), Error> {
            if let Some((data, status)) = self.byte_responses.get(url) {
                Ok((Bytes::from(data.clone()), *status))
            } else {
                Err(crate::shared::error::GenericSnafu {
                    reason: format!("No mock byte response for URL: {}", url),
                }
                .build())
            }
        }
    }

    #[tokio::test]
    async fn test_get_movie_details_success() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"adult":false,"backdrop_path":null,"budget":63000000,"genres":[],"homepage":null,"id":550,"imdb_id":"tt0137523","origin_country":["US"],"original_language":"en","original_title":"Fight Club","overview":"A test movie","popularity":8.4,"poster_path":"/poster.jpg","production_companies":[{"id":508,"logo_path":null,"name":"Fox 2000 Pictures","origin_country":"US"}],"release_date":"1999-10-15","revenue":100853753,"runtime":139,"status":"Released","tagline":"Mischief. Mayhem. Soap.","title":"Fight Club","video":false,"vote_average":8.4,"vote_count":27000}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/movie/550".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.get_movie_details("550").await;

        assert!(result.is_ok());
        let movie = result.unwrap();
        assert_eq!(movie.title, "Fight Club");
        assert_eq!(movie.id, 550);
    }

    #[tokio::test]
    async fn test_get_movie_details_invalid_json() {
        let mut mock = MockHttpClient::new();
        mock.set_response(
            "https://api.themoviedb.org/3/movie/123".to_string(),
            "invalid json".to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.get_movie_details("123").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_by_title_with_results() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"results":[{"id":550,"title":"Fight Club","original_title":"Fight Club","overview":"Test","poster_path":"/poster.jpg","backdrop_path":null,"release_date":"1999-10-15","popularity":8.4,"vote_average":8.4,"vote_count":27000,"adult":false,"genre_ids":[18,53],"video":false},{"id":551,"title":"Fight Club 2","original_title":"Fight Club 2","overview":"Test2","poster_path":null,"backdrop_path":null,"release_date":"2015-05-27","popularity":2.1,"vote_average":6.8,"vote_count":150,"adult":false,"genre_ids":[18],"video":false}]}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/search/movie".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.search_by_title("Fight Club").await;

        assert!(result.is_ok());
        let movies = result.unwrap();
        assert_eq!(movies.len(), 2);
        assert_eq!(movies[0].title, "Fight Club");
        assert_eq!(movies[1].title, "Fight Club 2");
    }

    #[tokio::test]
    async fn test_search_by_title_no_results() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"results":[]}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/search/movie".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.search_by_title("NonexistentMovie").await;

        assert!(result.is_ok());
        let movies = result.unwrap();
        assert_eq!(movies.len(), 0);
    }

    #[tokio::test]
    async fn test_search_by_title_missing_results_field() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"page":1,"total_results":0}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/search/movie".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.search_by_title("Test").await;

        assert!(result.is_ok());
        let movies = result.unwrap();
        assert_eq!(movies.len(), 0);
    }

    #[tokio::test]
    async fn test_search_by_imdb_id_found() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"results":[{"id":550,"title":"Fight Club","original_title":"Fight Club","overview":"Test","poster_path":"/poster.jpg","backdrop_path":null,"release_date":"1999-10-15","popularity":8.4,"vote_average":8.4,"vote_count":27000,"adult":false,"genre_ids":[18,53],"video":false}]}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/search/movie".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.search_by_imdb_id("tt0137523").await;

        assert!(result.is_ok());
        let movie = result.unwrap();
        assert!(movie.is_some());
        assert_eq!(movie.unwrap().title, "Fight Club");
    }

    #[tokio::test]
    async fn test_search_by_imdb_id_not_found() {
        let mut mock = MockHttpClient::new();
        let response_json = r#"{"results":[]}"#;
        mock.set_response(
            "https://api.themoviedb.org/3/search/movie".to_string(),
            response_json.to_string(),
        );

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.search_by_imdb_id("tt9999999").await;

        assert!(result.is_ok());
        let movie = result.unwrap();
        assert!(movie.is_none());
    }

    #[tokio::test]
    async fn test_get_poster_success() {
        let mut mock = MockHttpClient::new();
        let poster_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        mock.set_byte_response(
            "https://image.tmdb.org/t/p/w500/poster.jpg".to_string(),
            poster_bytes.clone(),
            200,
        );

        let tmdb_movie = TmdbMovie {
            adult: false,
            backdrop_path: None,
            budget: 63000000,
            genres: vec![],
            homepage: None,
            id: 550,
            imdb_id: Some("tt0137523".to_string()),
            origin_country: "US".to_string(),
            original_language: "en".to_string(),
            original_title: "Fight Club".to_string(),
            overview: "Test".to_string(),
            popularity: 8.4,
            poster_path: Some("/poster.jpg".to_string()),
            production_company: "Fox 2000 Pictures".to_string(),
            release_date: "1999-10-15".to_string(),
            revenue: 100853753,
            runtime: Some(139),
            status: "Released".to_string(),
            tagline: None,
            title: "Fight Club".to_string(),
            video: false,
            vote_average: 8.4,
            vote_count: 27000,
        };

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.get_poster(tmdb_movie, 550).await;

        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(bytes.to_vec(), poster_bytes);
    }

    #[tokio::test]
    async fn test_get_poster_no_poster_path() {
        let mock = MockHttpClient::new();

        let tmdb_movie = TmdbMovie {
            adult: false,
            backdrop_path: None,
            budget: 63000000,
            genres: vec![],
            homepage: None,
            id: 550,
            imdb_id: Some("tt0137523".to_string()),
            origin_country: "US".to_string(),
            original_language: "en".to_string(),
            original_title: "Fight Club".to_string(),
            overview: "Test".to_string(),
            popularity: 8.4,
            poster_path: None,
            production_company: "Fox 2000 Pictures".to_string(),
            release_date: "1999-10-15".to_string(),
            revenue: 100853753,
            runtime: Some(139),
            status: "Released".to_string(),
            tagline: None,
            title: "Fight Club".to_string(),
            video: false,
            vote_average: 8.4,
            vote_count: 27000,
        };

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.get_poster(tmdb_movie, 550).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_poster_http_error() {
        let mut mock = MockHttpClient::new();
        let poster_bytes = vec![];
        mock.set_byte_response(
            "https://image.tmdb.org/t/p/w500/poster.jpg".to_string(),
            poster_bytes,
            404,
        );

        let tmdb_movie = TmdbMovie {
            adult: false,
            backdrop_path: None,
            budget: 63000000,
            genres: vec![],
            homepage: None,
            id: 550,
            imdb_id: Some("tt0137523".to_string()),
            origin_country: "US".to_string(),
            original_language: "en".to_string(),
            original_title: "Fight Club".to_string(),
            overview: "Test".to_string(),
            popularity: 8.4,
            poster_path: Some("/poster.jpg".to_string()),
            production_company: "Fox 2000 Pictures".to_string(),
            release_date: "1999-10-15".to_string(),
            revenue: 100853753,
            runtime: Some(139),
            status: "Released".to_string(),
            tagline: None,
            title: "Fight Club".to_string(),
            video: false,
            vote_average: 8.4,
            vote_count: 27000,
        };

        let client = TmdbClient::with_client("test_api_key".to_string(), mock);
        let result = client.get_poster(tmdb_movie, 550).await;

        assert!(result.is_err());
    }
}
