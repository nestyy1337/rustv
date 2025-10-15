use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, Take};
use tokio::{fs::File, io::AsyncSeekExt};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Response},
    response::IntoResponse,
};
use sqlx::{Pool, Sqlite};
use tokio_util::io::ReaderStream;

use crate::{
    app::AppState,
    repositories::movies::MovieRepository,
    shared::{
        error::Error,
        middleware::{AuthBackendSqlite, AuthSession},
    },
};

async fn get_movie_path(movie_id: i64, pool: &Pool<Sqlite>) -> Result<PathBuf, Error> {
    let movies_dir = std::env::current_dir()
        .map_err(|e| {
            tracing::error!("Failed to get current directory: {}", e);
            Error::Generic(e.to_string())
        })?
        .join("movies");

    let movie = MovieRepository::get_movie_by_id(movie_id, pool)
        .await?
        .ok_or(Error::MovieNotFound)?;

    let final_movie_path = movies_dir.join(format!("{}.mp4", movie.id));
    tracing::info!("Resolved movie path: {}", final_movie_path.display());
    Ok(final_movie_path)
}

pub async fn parse_range_header(header: &str, file_size: u64) -> Result<Option<(u64, u64)>, Error> {
    println!("Parsing range header: {}", header);
    if !header.starts_with("bytes=") {
        return Ok(None);
    }
    let range = &header[6..];
    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return Ok(None);
    }
    let start: u64 = parts[0].parse().map_err(|_| Error::InvalidRange)?;
    let end: u64 = if parts[1].is_empty() {
        file_size - 1
    } else {
        parts[1].parse().map_err(|_| Error::InvalidRange)?
    };
    if start > end || end >= file_size {
        println!(
            "Invalid range: start={}, end={}, file_size={}",
            start, end, file_size
        );
        return Ok(None);
    }
    Ok(Some((start, end)))
}

pub struct VideoStream {
    pub stream: ReaderStream<Take<File>>,
    pub content_length: u64,
    pub file_size: u64,
    pub start: u64,
    pub end: u64,
}

pub struct StreamingService;

impl StreamingService {
    pub async fn stream_video(
        movie_id: i64,
        range_header: (u64, u64),
        pool: &Pool<Sqlite>,
    ) -> Result<VideoStream, Error> {
        let movie_path = get_movie_path(movie_id, pool).await?;
        println!("fine0");
        let file_size = tokio::fs::metadata(&movie_path).await;
        let file_size = match file_size {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                tracing::error!("Failed to get file metadata: {}", e);
                return Err(Error::MovieNotFound);
            }
        };
        println!("fine1");
        let file = tokio::fs::File::open(&movie_path).await;
        let mut file = match file {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to open movie file: {}", e);
                return Err(Error::MovieNotFound);
            }
        };
        let (start, end) = range_header;
        println!("fine2");

        let seek = file.seek(std::io::SeekFrom::Start(start)).await;
        if let Err(e) = seek {
            tracing::error!("Failed to seek in movie file: {}", e);
            return Err(Error::TokioIoError(e));
        }
        println!("fine3");

        let content_length = end - start + 1;
        let file = file.take(content_length);
        let stream = ReaderStream::new(file);
        Ok(VideoStream {
            stream,
            content_length,
            file_size,
            start,
            end,
        })
    }

    pub async fn file_size(pool: &Pool<Sqlite>, movie_id: i64) -> Result<u64, Error> {
        let movie_path = get_movie_path(movie_id, pool).await?;
        let file_size = tokio::fs::metadata(&movie_path).await;
        let file_size = match file_size {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                tracing::error!("Failed to get file metadata: {}", e);
                return Err(Error::MovieNotFound);
            }
        };
        Ok(file_size)
    }
}
