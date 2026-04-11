use reqwest::Url;
use std::path::PathBuf;

use crate::services::movie_manager::MovieManager;

pub enum IndexLocation {
    Local(PathBuf),
    Remote(Url),
}

pub enum SegmentLocation {
    Local(Vec<u8>),
    Remote(String),
}

#[async_trait::async_trait]
pub trait StreamingService {}

pub struct SimpleStreamingService {
    movie_manager: MovieManager,
}

impl SimpleStreamingService {
    pub fn new(movie_manager: MovieManager) -> Self {
        Self { movie_manager }
    }
}

#[async_trait::async_trait]
impl StreamingService for SimpleStreamingService {}
