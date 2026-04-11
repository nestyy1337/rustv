use reqwest::Url;
use std::path::PathBuf;

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

pub struct SimpleStreamingService {}

impl SimpleStreamingService {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl std::default::Default for SimpleStreamingService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl StreamingService for SimpleStreamingService {}
