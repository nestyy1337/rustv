use anyhow::Result;
use reqwest::{Client, Response};

pub struct TestClient {
    client: Client,
    base_url: String,
}

impl TestClient {
    pub fn new(addr: &str) -> Self {
        Self {
            client: Client::builder().cookie_store(true).build().unwrap(),
            base_url: addr.to_string(),
        }
    }

    pub async fn login_default(&self) -> Result<Response> {
        self.login("ferris", "hunter42").await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<Response> {
        self.client
            .post(&format!("{}/login", self.base_url))
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(Into::into)
    }

    pub async fn get(&self, path: &str) -> Result<Response> {
        self.client
            .get(&format!("{}{}", self.base_url, path))
            .send()
            .await
            .map_err(Into::into)
    }

    pub async fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(&format!("{}{}", self.base_url, path))
    }

    pub async fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.delete(&format!("{}{}", self.base_url, path))
    }
}
