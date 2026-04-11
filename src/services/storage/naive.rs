use std::path::PathBuf;

use snafu::ResultExt;
use sqlx::{Pool, Sqlite};

use crate::{
    models::movie::{MoviePath, MovieState},
    repositories::movies::MovieRepository,
    services::{
        converter::{ConvertedVideo, HLS, StreamableVideoFormat},
        movie_manager::{StreamableVideo, VIDEOABLE_EXE_EXTENSIONS},
        streaming::{IndexLocation, SegmentLocation},
    },
    shared::error::{
        Error, MovieError, MovieMissingReason, MovieNotAvailableSnafu, MovieNotFoundSnafu,
        MovieSegmentSnafu, TokioIoSnafu,
    },
};

use super::{MovieStorage, MovieStoragePaths};

const DOWNLOADS_PATH: &str = "./downloads/";
const MOVIES_PATH: &str = "./movies/";

#[derive(Clone)]
pub struct NaiveMovieStorage {
    downloads_path: String,
    movies_path: String,
    pool: Pool<Sqlite>,
}

impl NaiveMovieStorage {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        NaiveMovieStorage {
            downloads_path: DOWNLOADS_PATH.to_string(),
            movies_path: MOVIES_PATH.to_string(),
            pool,
        }
    }
    pub fn with_paths(downloads_path: String, movies_path: String, pool: Pool<Sqlite>) -> Self {
        NaiveMovieStorage {
            downloads_path,
            movies_path,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl MovieStorage for NaiveMovieStorage {
    fn downloads_path(&self) -> &str {
        &self.downloads_path
    }

    async fn delete_movie(&self, movie_id: i64) -> Result<(), Error> {
        let movie_dir = PathBuf::from(format!("{}/{}", self.movies_path, movie_id));
        if movie_dir.exists() && movie_dir.is_dir() {
            let mut entries = tokio::fs::read_dir(&movie_dir)
                .await
                .context(TokioIoSnafu {
                    operation: "reading movie directory",
                })?;
            while let Some(entry) = entries.next_entry().await.context(TokioIoSnafu {
                operation: "reading movie directory entry",
            })? {
                let path = entry.path();
                if path.is_file()
                    && VIDEOABLE_EXE_EXTENSIONS
                        .iter()
                        .any(|&ext| path.extension().is_some_and(|file_ext| file_ext == ext))
                {
                    tokio::fs::remove_file(path).await.context(TokioIoSnafu {
                        operation: "deleting movie file from directory",
                    })?;
                }
            }
        }
        tracing::info!(
            movie_id = movie_id, movie_path = ?movie_dir,
            "Deleted streamable movie directory contents"
        );
        Ok(())
    }

    async fn segment_bytes(
        &self,
        movie_id: i64,
        segment: crate::handlers::movies::HlsSegmentUnchecked,
    ) -> Result<SegmentLocation, Error> {
        let validated = segment
            .validate()
            .map_err(|_| MovieSegmentSnafu { movie_id }.build())?;

        let _movie = MovieRepository::get_movie_by_id(movie_id, &self.pool)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;
        let streamable = self
            .get_streamable(movie_id)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;
        let path = if let MoviePath::Local(local_path) = &streamable.path {
            local_path
        } else {
            unreachable!("In NaiveMovieStorage, streamable videos should always have local paths")
        };
        let segment_path = path.join(validated.as_ref());

        let bytes = tokio::fs::read(&segment_path).await.context(TokioIoSnafu {
            operation: "reading segment file",
        })?;

        Ok(SegmentLocation::Local(bytes))
    }

    async fn get_streamable(&self, movie_id: i64) -> Result<Option<StreamableVideo>, Error> {
        let movie = MovieRepository::get_movie_by_id(movie_id, &self.pool).await?;
        if movie.is_none() {
            return Ok(None);
        }
        let movie_path = PathBuf::from(format!("{}{}/", self.movies_path, movie_id));
        println!("Checking for streamable content at path: {:?}", movie_path);
        if !movie_path.exists() || !movie_path.is_dir() {
            return Ok(None);
        }

        let streamable = StreamableVideo {
            path: MoviePath::Local(movie_path),
            formats: vec![StreamableVideoFormat::HLS(HLS::default())],
            movie: movie.unwrap(),
        };
        return Ok(Some(streamable));
    }

    async fn get_m3u8_content(&self, movie_id: i64) -> Result<IndexLocation, Error> {
        let streamable = self
            .get_streamable(movie_id)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;

        let path = if let MoviePath::Local(local_path) = &streamable.path {
            local_path
        } else {
            unreachable!("In NaiveMovieStorage, streamable videos should always have local paths")
        };

        let m3u8_path = path.join("index.m3u8");
        if !m3u8_path.exists() {
            return Err(MovieNotFoundSnafu {
                movie_id,
                reason: MovieMissingReason::NoFile,
            }
            .build()
            .into());
        }

        Ok(IndexLocation::Local(m3u8_path))
    }

    async fn get_poster(&self, movie_id: i64) -> Result<Option<Vec<u8>>, Error> {
        let poster_path = PathBuf::from(format!("{}{}/poster.jpg", self.movies_path, movie_id));
        if poster_path.exists() && poster_path.is_file() {
            let bytes = tokio::fs::read(poster_path).await.context(TokioIoSnafu {
                operation: "reading poster file",
            })?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }

    async fn verify_movie_content(&self, movie_id: i64) -> Result<(), Error> {
        let dir_path = PathBuf::from(format!("{}{}/", self.movies_path, movie_id));
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(MovieNotAvailableSnafu {
                movie_id: 0,
                movie_state: MovieState::Unavailable,
            }
            .build())?;
        }

        let mut entries = tokio::fs::read_dir(dir_path).await.context(TokioIoSnafu {
            operation: "reading directory",
        })?;

        let mut has_valid_files = false;

        while let Some(entry) = entries.next_entry().await.context(TokioIoSnafu {
            operation: "reading directory entry",
        })? {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension()
                    && (ext == "m3u8" || ext == "mpd")
                {
                    has_valid_files = true;
                    break;
                }
            }
        }

        if has_valid_files {
            Ok(())
        } else {
            Err(MovieNotAvailableSnafu {
                movie_id: 0,
                movie_state: MovieState::Unavailable,
            }
            .build())?
        }
    }

    async fn save_converted(&self, converted: ConvertedVideo) -> Result<StreamableVideo, Error> {
        let converted_path =
            PathBuf::from(&self.downloads_path).join(format!("{}", converted.movie.id));
        let movie_path = PathBuf::from(&self.movies_path).join(format!("{}", converted.movie.id));
        tokio::fs::create_dir_all(&movie_path)
            .await
            .context(TokioIoSnafu {
                operation: "creating converted video directory",
            })?;
        let mut entries = tokio::fs::read_dir(&converted_path)
            .await
            .context(TokioIoSnafu {
                operation: "reading converted videos directory",
            })?;
        while let Some(entry) = entries.next_entry().await.context(TokioIoSnafu {
            operation: "reading converted video entry",
        })? {
            if !entry.path().is_file() {
                continue;
            }
            tracing::info!(
                "Saving converted file {:?} to {:?}",
                entry.path(),
                movie_path
            );
            println!(
                "Saving converted file {:?} to {:?}",
                entry.path(),
                movie_path
            );
            tokio::fs::copy(entry.path(), &movie_path.join(entry.file_name()))
                .await
                .context(TokioIoSnafu {
                    operation: "copying converted file to converted videos directory",
                })?;
        }

        Ok(StreamableVideo {
            path: MoviePath::Local(movie_path),
            formats: vec![StreamableVideoFormat::HLS(HLS::default())],
            movie: converted.movie,
        })
    }
}
