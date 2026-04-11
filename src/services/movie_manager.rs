use std::{collections::HashMap, path::PathBuf, sync::Arc};

use snafu::ResultExt;
use sqlx::{Pool, Sqlite, SqlitePool};
use tokio::sync::RwLock;

use crate::{
    clients::tmdb::{ReqwestHttpClient, TmdbClient},
    handlers::movies::HlsSegmentUnchecked,
    models::{
        movie::{Movie, MoviePath, MovieState, Watchlist},
        users::UserProfile,
    },
    repositories::movies::MovieRepository,
    services::{
        converter::{
            ConvertedVideo, Converter, RawVideoFormat, StreamableFormat, StreamableVideoFormat,
            VideoFile,
        },
        storage::MovieStorage,
        streaming::{IndexLocation, SegmentLocation},
        torrent::{ActiveDownload, ActiveProcessing},
    },
    shared::{
        config::SETTINGS,
        error::{Error, MovieError, TokioIoSnafu},
    },
};

use super::movies::MovieService;

pub const VIDEOABLE_EXE_EXTENSIONS: [&str; 6] = ["mkv", "mp4", "avi", "ts", "m3u8", "mpd"];

#[derive(Debug)]
pub struct MovieManagerState {
    pub available_movies: Vec<Movie>,
}

#[derive(Clone)]
pub struct MovieManager {
    pub inner: Arc<RwLock<MovieManagerState>>,
    pub movie_service: Arc<dyn MovieService + Send + Sync>,
    storage: Arc<dyn MovieStorage + Send + Sync>,
    tmdb_client: Arc<TmdbClient<ReqwestHttpClient>>,
    pub pool: Pool<Sqlite>,
}

#[derive(Debug)]
pub struct StreamableVideo {
    pub path: MoviePath,
    pub formats: Vec<StreamableVideoFormat>,
    pub movie: Movie,
}

#[derive(Debug)]
pub struct DownloadedVideo {
    pub path: PathBuf,
    pub formats: Vec<RawVideoFormat>,
    pub movie: Movie,
}

#[derive(Debug)]
pub enum DirectoryType {
    Downloaded,
    Movies,
}

impl MovieManager {
    pub fn downloads_path(&self) -> &str {
        "./downloads/"
    }

    pub async fn get_poster(&self, movie_id: i64) -> Result<Option<Vec<u8>>, Error> {
        if let Some(poster) = self.storage.get_poster(movie_id).await? {
            return Ok(Some(poster));
        }

        let movie = MovieRepository::get_movie_by_id(movie_id, &self.pool)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;

        let tmdb_movie = if let Some(tmdb_movie) =
            MovieRepository::get_tmdb_movie_by_imdb_id(&movie.imdb_id, &self.pool).await?
        {
            tmdb_movie
        } else if let Some(tmdb_match) = self.tmdb_client.search_by_imdb_id(&movie.imdb_id).await? {
            self.tmdb_client
                .get_movie_details(&tmdb_match.id.to_string())
                .await?
        } else {
            return Ok(None);
        };

        let poster = self.tmdb_client.get_poster(tmdb_movie, movie_id).await?;
        Ok(Some(poster.to_vec()))
    }

    pub async fn complete_download(
        &self,
        download: ActiveDownload,
        active_processing: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
        converter: &dyn Converter<Error = Error>,
    ) -> Result<(), Error> {
        tracing::info!(
            movie_id = download.movie.id,
            "Starting download completion pipeline"
        );

        let downloaded_video = self.convert_downloaded(download).await?;
        tracing::info!(
            movie_id = downloaded_video.movie.id,
            "Converted downloaded video to organized structure"
        );
        let streamable = self
            .process_downloaded_movie(downloaded_video, active_processing, converter)
            .await?;
        tracing::info!(
            movie_id = streamable.movie.id,
            "Processed downloaded movie to streamable format"
        );
        self.add_available_movie(streamable).await?;

        Ok(())
    }

    pub async fn segment_bytes(
        &self,
        movie_id: i64,
        segment: HlsSegmentUnchecked,
    ) -> Result<SegmentLocation, Error> {
        self.storage.segment_bytes(movie_id, segment).await
    }

    // #[tracing::instrument(name = "verifying downloaded movie", skip(self, movie))]
    // async fn verify_downloaded_movie(&self, movie: &Movie) -> Result<(), Error> {
    //     let downloaded_movie_dir = PathBuf::from(format!("{}{}/", self.downloads_path(), movie.id));
    //     if !downloaded_movie_dir.exists() {
    //         tracing::error!(
    //             "Downloaded movie directory does not exist for movie ID: {} at path: {:?}",
    //             movie.id,
    //             downloaded_movie_dir
    //         );
    //         return Err(MovieError::SimpleMovieNotAvailable.into());
    //     }
    //
    //     self.verity_directory_content(&downloaded_movie_dir, DirectoryType::Downloaded)
    //         .await?;
    //     Ok(())
    // }

    #[tracing::instrument(name = "verifying downloaded movie", skip(self, movie))]
    async fn verify_downloaded_movie(&self, movie: &Movie) -> Result<(), Error> {
        self.storage.verify_movie_content(movie.id).await?;
        Ok(())
    }

    pub async fn get_m3u8_content(&self, movie_id: i64) -> Result<IndexLocation, Error> {
        self.storage.get_m3u8_content(movie_id).await
    }

    pub async fn is_watched(&self, user_id: i64, movie_id: i64) -> Result<bool, Error> {
        self.movie_service.is_watched(user_id, movie_id).await
    }

    pub async fn is_watchlisted(&self, user_id: i64, movie_id: i64) -> Result<bool, Error> {
        self.movie_service.is_watchlisted(user_id, movie_id).await
    }

    #[tracing::instrument(name = "adding available movie", skip(self, streamable))]
    pub async fn add_available_movie(&self, streamable: StreamableVideo) -> Result<(), Error> {
        let movie = streamable.movie;
        let id = movie.id;
        if self.verify_downloaded_movie(&movie).await.is_err() {
            tracing::error!(
                "Downloaded movie ID: {} failed verification, cannot add to available movies",
                movie.id
            );
            return Err(MovieError::SimpleMovieNotAvailable.into());
        }
        tracing::info!(movie_id = movie.id, "Adding movie to available movies");
        MovieRepository::mark_available(movie.id, &self.pool).await?;
        self.inner.write().await.available_movies.push(movie);
        tracing::info!(movie_id = id, "Added movie to available movies");
        Ok(())
    }

    pub async fn add_watched_movie(
        &self,
        user_id: i64,
        movie_id: i64,
        rating: Option<f32>,
    ) -> Result<(), Error> {
        self.movie_service
            .add_watched_movie(user_id, movie_id, rating)
            .await
    }

    async fn mark_removed(&self, movie_id: i64) -> Result<(), Error> {
        // more validation here
        self.inner
            .write()
            .await
            .available_movies
            .retain(|m| m.id != movie_id);
        Ok(())
    }

    pub async fn add_watchlisted_movie(&self, user_id: i64, movie_id: i64) -> Result<(), Error> {
        self.movie_service
            .add_watchlisted_movie(user_id, movie_id)
            .await
    }

    pub async fn remove_from_watchlist(&self, user_id: i64, movie_id: i64) -> Result<(), Error> {
        self.movie_service
            .remove_from_watchlist(user_id, movie_id)
            .await
    }
    // this is the outer wrapper that allows us to do all the necessary steps to remove a movie
    // including the 'runtime' side which is the representation of available movies
    // as well as the database side which is handled by the movie service
    // and any storage system that is used (fs or something like s3)
    pub async fn remove_movie(&self, movie_id: i64) -> Result<(), Error> {
        self.mark_removed(movie_id).await?;
        self.movie_service.delete_movie(movie_id).await
    }

    pub async fn get_user_watchlist(
        &self,
        username: &str,
    ) -> Result<(UserProfile, Vec<Watchlist>), Error> {
        self.movie_service.get_user_watchlist(username).await
    }

    pub async fn get_available_movies(&self) -> Vec<Movie> {
        self.inner.read().await.available_movies.clone()
    }

    pub async fn convert_downloaded(
        &self,
        download: ActiveDownload,
    ) -> Result<DownloadedVideo, Error> {
        tracing::info!(
            movie_id = download.movie.id,
            "Converting downloaded torrent files to organized structure"
        );

        let target_dir = PathBuf::from(self.downloads_path()).join(download.movie.id.to_string());
        tokio::fs::create_dir_all(&target_dir)
            .await
            .context(TokioIoSnafu {
                operation: "creating movie download directory",
            })?;

        let video_file = self
            .find_video_file_in_directory(
                PathBuf::from(self.downloads_path())
                    .join(PathBuf::from(download.handle.name().unwrap_or_default())),
            )
            .await?;

        tracing::info!(
            movie_id = download.movie.id,
            source = ?video_file,
            "Found video file in torrent download"
        );

        let extension = video_file
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| {
                crate::shared::error::GenericSnafu {
                    reason: "video file has no extension".to_string(),
                }
                .build()
            })?;

        let format = RawVideoFormat::from_extension(extension).ok_or_else(|| {
            crate::shared::error::UnsupportedFormatSnafu {
                format: extension.to_string(),
            }
            .build()
        })?;

        let target_file = target_dir.join(format!("video.{}", extension));

        tracing::info!(
            movie_id = download.movie.id,
            source = ?video_file,
            target = ?target_file,
            "Moving video file to organized structure"
        );

        tokio::fs::copy(&video_file, &target_file)
            .await
            .context(TokioIoSnafu {
                operation: "copying video file to movie directory",
            })?;

        tracing::info!(
            movie_id = download.movie.id,
            final_path = ?target_file,
            "Successfully organized downloaded movie"
        );

        Ok(DownloadedVideo {
            path: target_file,
            formats: vec![format],
            movie: download.movie,
        })
    }

    async fn find_video_file_in_directory(&self, dir: PathBuf) -> Result<PathBuf, Error> {
        if dir.is_file() {
            return Ok(dir);
        }
        let mut entires = tokio::fs::read_dir(&dir).await.context(TokioIoSnafu {
            operation: "reading torrent download directory",
        })?;
        while let Some(entry) = entires.next_entry().await.context(TokioIoSnafu {
            operation: "reading torrent download directory entry",
        })? {
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension()
                    && VIDEOABLE_EXE_EXTENSIONS.contains(&ext.to_str().unwrap_or(""))
                {
                    return Ok(path);
                }
            } else if path.is_dir() {
                while let Some(nested_entry) = tokio::fs::read_dir(&path)
                    .await
                    .context(TokioIoSnafu {
                        operation: "reading nested directory in torrent download",
                    })?
                    .next_entry()
                    .await
                    .context(TokioIoSnafu {
                        operation: "reading nested directory entry",
                    })?
                {
                    let nested_path = nested_entry.path();

                    if nested_path.is_file()
                        && let Some(ext) = nested_path.extension()
                        && VIDEOABLE_EXE_EXTENSIONS.contains(&ext.to_str().unwrap_or(""))
                    {
                        return Ok(nested_path);
                    }
                }
            }
        }

        Err(crate::shared::error::NoValidVideoFileSnafu {
            torrent_id: dir.to_string_lossy().to_string(),
        }
        .build()
        .into())
    }

    pub async fn process_downloaded_movie(
        &self,
        downloaded: DownloadedVideo,
        processing: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
        converter: &dyn crate::services::converter::Converter<Error = Error>,
    ) -> Result<StreamableVideo, Error> {
        let movie = downloaded.movie.clone();
        let id = movie.id;
        tracing::info!(
            movie_id = movie.id,
            "Processing downloaded movie to available movies"
        );
        let movie = MovieRepository::get_movie_by_id(movie.id, &self.pool)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;

        let videofile = VideoFile::try_from(downloaded.path)?;

        // if !converter.supports(&videofile.codec, &StreamableFormat::HLS) {
        //     return Err(UnsupportedConversionSnafu {
        //         from: videofile.codec.to_string(),
        //         to: StreamableFormat::HLS.to_string(),
        //     }
        //     .build())?;
        // }

        let converted = converter
            .convert(&videofile, StreamableFormat::HLS, movie, processing)
            .await?;

        tracing::info!(
            movie_id = id,
            "Finished converting downloaded movie to streamable format"
        );

        let streamable = self.storage.save_converted(converted).await?;
        tracing::info!(
            movie_id = id,
            "Saved converted movie to storage and created streamable video representation"
        );

        Ok(streamable)
    }

    pub async fn get_streamable(&self, movie_id: i64) -> Result<Option<StreamableVideo>, Error> {
        let streamable = self.storage.get_streamable(movie_id).await?;
        Ok(streamable)
    }
}

impl MovieManager {
    #[tracing::instrument(
        name = "initializing movie manager",
        skip(pool, storage, movie_service)
    )]
    pub async fn initialize(
        movie_service: Arc<dyn MovieService + Send + Sync>,
        storage: Arc<dyn MovieStorage + Send + Sync>,
        pool: &SqlitePool,
    ) -> Self {
        tracing::info!("Initializing Movie Manager");

        let manager_state = MovieManagerState {
            available_movies: Vec::new(),
        };
        let tmdb_client = Arc::new(TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone()));
        let mut manager = MovieManager {
            inner: Arc::new(RwLock::new(manager_state)),
            storage,
            tmdb_client,
            movie_service: movie_service.clone(),
            pool: pool.clone(),
        };
        manager.verify_base_paths().await;
        tracing::info!("Verified base paths for downloads and movies");

        let movies = MovieRepository::get_all_movies(pool)
            .await
            .expect("failed to fetch all movies from database, cannot initialize Movie Manager");

        for movie in movies.iter() {
            if manager.verify_downloaded_movie(movie).await.is_err() {
                let download_path =
                    PathBuf::from(format!("{}{}/", manager.storage.downloads_path(), movie.id));
                manager.cleanup_unfinished_download(&download_path).await;
                tracing::info!(
                    "Cleaned up unfinished download for movie ID: {} at path: {:?}",
                    movie.id,
                    download_path
                );
            }
        }

        manager.find_available_movies().await.unwrap_or_default();
        tracing::info!("Finished initializing Movie Manager");
        manager
    }

    #[tracing::instrument(name = "finding available movies", skip(self))]
    pub async fn find_available_movies(&mut self) -> Result<(), Error> {
        let movies = MovieRepository::get_all_available_movies(&self.pool).await?;
        for movie in movies {
            tracing::debug!(
                "Movie ID: {} passed verification, adding to available movies",
                movie.id
            );
            self.inner.write().await.available_movies.push(movie);
        }

        tracing::info!("Finished finding available movies");

        Ok(())
    }

    #[tracing::instrument(name = "removing available movie", skip(self))]
    pub async fn remove_available_movie(&mut self, movie_id: i64) -> Result<(), Error> {
        self.inner
            .write()
            .await
            .available_movies
            .retain(|movie| movie.id != movie_id);
        self.storage.delete_movie(movie_id).await?;
        MovieRepository::delete_movie_cascade(&self.pool, movie_id).await?;

        tracing::info!(movie_id = movie_id, "Removed streamable movie directory");
        Ok(())
    }

    async fn cleanup_unfinished_download(&self, path: &PathBuf) {
        if path.exists() && path.is_dir() {
            let _ = tokio::fs::remove_dir_all(path).await;
        }
    }

    async fn verify_base_paths(&self) {
        let _ = tokio::fs::create_dir_all(&self.downloads_path()).await;
    }
}

#[cfg(any(test, feature = "integration-tests"))]
impl MovieManager {
    pub fn create_test() -> Self {
        use crate::shared::test_utils::TempDir;

        let pool = SqlitePool::connect_lazy(":memory:").expect("Failed to create in-memory DB");
        let movie_service = Arc::new(crate::services::movies::SimpleMovieService::new(
            pool.clone(),
        ));

        let tmdb_client = Arc::new(TmdbClient::new("test_api_key".to_string()));

        let download_path = TempDir::create()
            .expect("Failed to create temp dir for downloads")
            .path()
            .to_str()
            .unwrap()
            .to_string();

        let movies_path = TempDir::create()
            .expect("Failed to create temp dir for movies")
            .path()
            .to_str()
            .unwrap()
            .to_string();
        let naive_storage = crate::services::storage::naive::NaiveMovieStorage::with_paths(
            download_path,
            movies_path,
            pool.clone(),
        );

        MovieManager {
            inner: Arc::new(RwLock::new(MovieManagerState {
                available_movies: Vec::new(),
            })),
            movie_service,
            tmdb_client,
            storage: Arc::new(naive_storage),
            pool,
        }
    }
}

#[allow(unused_imports)]
mod tests {

    use std::path::PathBuf;

    use crate::{
        models::movie::MovieState,
        services::movie_manager::{DirectoryType, MovieManager},
        shared::test_utils::TempDir,
    };

    #[tokio::test]
    async fn test_verify_directory_content() {
        let movie_manager = MovieManager::create_test();

        let temp_dir = TempDir::create().expect("Failed to create temp dir");
        let dir_path = temp_dir.path();

        let valid_file_path = dir_path.join("video.mkv");
        std::fs::write(&valid_file_path, b"dummy data").expect("Failed to write file");
        let result = movie_manager
            .verity_directory_content(&dir_path.to_path_buf(), DirectoryType::Downloaded)
            .await;
        assert!(result.is_ok(), "Expected directory to be valid");
    }

    #[tokio::test]
    async fn test_verify_directory_content_invalid() {
        let manager = MovieManager::create_test();

        let temp_dir = TempDir::create().expect("Failed to create temp dir");
        let dir_path = temp_dir.path();
        let invalid_file_path = dir_path.join("document.txt");
        std::fs::write(&invalid_file_path, b"dummy data").expect("Failed to write file");
        let result = manager
            .verity_directory_content(&dir_path.to_path_buf(), DirectoryType::Downloaded)
            .await;
        assert!(result.is_err(), "Expected directory to be invalid");
    }

    #[tokio::test]
    async fn test_verify_directory_content_empty() {
        let manager = MovieManager::create_test();

        let temp_dir = TempDir::create().expect("Failed to create temp dir");
        let dir_path = temp_dir.path();
        let result = manager
            .verity_directory_content(&dir_path.to_path_buf(), DirectoryType::Downloaded)
            .await;
        assert!(result.is_err(), "Expected directory to be invalid");
    }

    #[tokio::test]
    async fn test_delete_movie_directory() {
        let manager = MovieManager::create_test();
        let movie_id = 12345;
        let movie_dir = PathBuf::from(format!("{}/{}", manager.movies_path(), movie_id));
        std::fs::create_dir_all(&movie_dir).expect("Failed to create movie directory");
        let video_file_path = movie_dir.join("video.mkv");
        std::fs::write(&video_file_path, b"dummy data").expect("Failed to write video file");
        let result = manager.delete_movie_directory(movie_id).await;
        assert!(result.is_ok(), "Expected deletion to succeed");
        assert!(
            !video_file_path.exists(),
            "Expected video file to be deleted"
        );
    }
    #[tokio::test]
    async fn test_delete_movie_directory_nonexistent() {
        let manager = MovieManager::create_test();
        let movie_id = 67890;
        let result = manager.delete_movie_directory(movie_id).await;
        assert!(
            result.is_ok(),
            "Expected deletion to succeed for nonexistent directory"
        );
    }

    #[tokio::test]
    async fn test_verify_base_paths() {
        let manager = MovieManager::create_test();
        manager.verify_base_paths().await;
        assert!(std::path::Path::new(&manager.downloads_path()).exists());
        assert!(std::path::Path::new(&manager.movies_path()).exists());
    }

    #[tokio::test]
    async fn test_cleanup_unfinished_download() {
        let manager = MovieManager::create_test();
        let temp_dir = TempDir::create().expect("Failed to create temp dir");
        let dir_path = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(&dir_path).expect("Failed to create directory");
        let file_path = dir_path.join("temp_file.txt");
        std::fs::write(&file_path, b"dummy data").expect("Failed to write file");
        manager.cleanup_unfinished_download(&dir_path).await;
        assert!(!dir_path.exists(), "Expected directory to be deleted");
    }
}
