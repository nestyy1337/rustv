use reqwest::Url;
use snafu::ResultExt;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::fs::File;
use tokio::io::Take;

use tokio_util::io::ReaderStream;

use crate::services::movie_manager::MovieManager;
use crate::shared::error::IndexFileNotFoundSnafu;
use crate::shared::error::IoSnafu;
use crate::shared::error::TokioIoSnafu;
use crate::shared::error::{Error, MovieMissingReason, MovieNotFoundSnafu};

use super::movie_manager::MovieStoreage;

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

pub struct SimpleStreamingService<T: MovieStoreage + Send + Sync + Clone> {
    movie_manager: MovieManager<T>,
}

impl<T: MovieStoreage + Send + Sync + Clone> SimpleStreamingService<T> {
    pub fn new(movie_manager: MovieManager<T>) -> Self {
        Self { movie_manager }
    }
}

#[async_trait::async_trait]
impl<T: MovieStoreage + Send + Sync + Clone> StreamingService for SimpleStreamingService<T> {}
