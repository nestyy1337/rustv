#[cfg(feature = "fs")]
pub mod naive;
#[cfg(feature = "s3")]
pub mod s3;

use crate::{
    handlers::movies::HlsSegmentUnchecked,
    services::{
        converter::ConvertedVideo,
        streaming::{IndexLocation, SegmentLocation},
    },
    shared::error::Error,
};

use super::movie_manager::StreamableVideo;

#[async_trait::async_trait]
pub trait MovieStorage: Send + Sync {
    fn downloads_path(&self) -> &str;
    async fn get_m3u8_content(&self, movie_id: i64) -> Result<IndexLocation, Error>;
    async fn get_streamable(&self, movie_id: i64) -> Result<Option<StreamableVideo>, Error>;
    async fn segment_bytes(
        &self,
        movie_id: i64,
        segment: HlsSegmentUnchecked,
    ) -> Result<SegmentLocation, Error>;
    async fn get_poster(&self, movie_id: i64) -> Result<Option<Vec<u8>>, Error>;
    async fn verify_movie_content(&self, movie_id: i64) -> Result<(), Error>;
    async fn save_converted(&self, converted: ConvertedVideo) -> Result<StreamableVideo, Error>;
    async fn delete_movie(&self, movie_id: i64) -> Result<(), Error>;
}
