use std::path::PathBuf;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use reqwest::Url;
use snafu::ResultExt;
use sqlx::{Pool, Sqlite};

use crate::{
    models::movie::MoviePath,
    repositories::movies::MovieRepository,
    services::{
        converter::{ConvertedVideo, HLS, StreamableVideoFormat},
        movie_manager::StreamableVideo,
        storage::MovieStorage,
        streaming::{IndexLocation, SegmentLocation},
    },
    shared::error::{AwsSDKSnafu, Error, MovieError, MovieSegmentSnafu, TokioIoSnafu},
};

pub struct S3MovieStorage {
    client: aws_sdk_s3::Client,
    bucket: String,
    pool: Pool<Sqlite>,
}

impl S3MovieStorage {
    pub async fn new(bucket: String, region: String, pool: Pool<Sqlite>) -> Self {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region))
            .load()
            .await;
        let client = aws_sdk_s3::Client::new(&config);
        Self {
            client,
            bucket,
            pool,
        }
    }
}

#[async_trait::async_trait]
impl MovieStorage for S3MovieStorage {
    fn downloads_path(&self) -> &'static str {
        "./downloads/"
    }
    async fn delete_movie(&self, movie_id: i64) -> Result<(), Error> {
        let prefix = format!("movies/{movie_id}/");
        let mut continuation_token = None;

        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&prefix);

            if let Some(token) = continuation_token.take() {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                .context(AwsSDKSnafu {
                    operation: "list movie objects for deletion",
                })?;

            let objects = response
                .contents()
                .iter()
                .filter_map(|object| object.key().map(str::to_owned))
                .map(|key| {
                    ObjectIdentifier::builder()
                        .key(key)
                        .build()
                        .expect("object identifier key should be set")
                })
                .collect::<Vec<_>>();

            if !objects.is_empty() {
                self.client
                    .delete_objects()
                    .bucket(&self.bucket)
                    .delete(
                        Delete::builder()
                            .set_objects(Some(objects))
                            .build()
                            .expect("delete payload should be valid"),
                    )
                    .send()
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                    .context(AwsSDKSnafu {
                        operation: "delete movie objects from S3",
                    })?;
            }

            if !response.is_truncated().unwrap_or(false) {
                break;
            }

            continuation_token = response.next_continuation_token().map(str::to_owned);
        }

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
        let segment_key = format!("movies/{}/{}", movie_id, validated.as_ref());

        // presigned
        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(segment_key)
            .presigned(
                PresigningConfig::builder()
                    .expires_in(std::time::Duration::from_secs(3600))
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "generate presigned URL for movie segment",
            })?;

        Ok(SegmentLocation::Remote(presigned_request.uri().to_string()))
    }

    async fn get_streamable(&self, movie_id: i64) -> Result<Option<StreamableVideo>, Error> {
        let movie = MovieRepository::get_movie_by_id(movie_id, &self.pool).await?;
        if movie.is_none() {
            return Ok(None);
        }
        let streamable = StreamableVideo {
            path: MoviePath::Remote(movie_id.to_string()),
            formats: vec![StreamableVideoFormat::HLS(HLS::default())],
            movie: movie.unwrap(),
        };
        return Ok(Some(streamable));
    }

    async fn get_m3u8_content(&self, movie_id: i64) -> Result<IndexLocation, Error> {
        let _streamable = self
            .get_streamable(movie_id)
            .await?
            .ok_or(MovieError::SimpleMovieNotFound)?;

        let presigned = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(format!("movies/{movie_id}/index.m3u8"))
            .presigned(
                PresigningConfig::builder()
                    .expires_in(std::time::Duration::from_secs(3600))
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "generate presigned URL for movie index",
            })?;

        Ok(IndexLocation::Remote(
            Url::parse(presigned.uri().to_string().as_str()).unwrap(),
        ))
    }
    async fn get_poster(&self, movie_id: i64) -> Result<Option<Vec<u8>>, Error> {
        let key = format!("movies/{movie_id}/poster.jpg");
        let exists = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&key)
            .max_keys(1)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "check poster exists in S3",
            })?
            .contents()
            .iter()
            .any(|object| object.key().is_some_and(|object_key| object_key == key));

        if !exists {
            return Ok(None);
        }

        let bytes = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "get poster from S3",
            })?
            .body
            .collect()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "read poster bytes from S3 response",
            })?;
        Ok(Some(bytes.to_vec()))
    }

    async fn verify_movie_content(&self, movie_id: i64) -> Result<(), Error> {
        let key = format!("movies/{movie_id}/index.m3u8");
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            .context(AwsSDKSnafu {
                operation: "verify movie content exists in S3",
            })?;
        Ok(())
    }

    async fn save_converted(&self, converted: ConvertedVideo) -> Result<StreamableVideo, Error> {
        let converted_path =
            PathBuf::from(&self.downloads_path()).join(format!("{}", converted.movie.id));

        let movie_path = format!("movies/{}/", converted.movie.id);
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
            if !entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "m3u8" || ext == "ts")
            {
                continue;
            }

            tracing::info!(
                "Saving converted file {:?} to {:?}",
                entry.path(),
                movie_path
            );
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_bytes = tokio::fs::read(entry.path()).await.context(TokioIoSnafu {
                operation: "reading converted video file bytes",
            })?;

            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(format!("{movie_path}{file_name}"))
                .body(file_bytes.into())
                .send()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                .context(AwsSDKSnafu {
                    operation: "upload converted video to S3",
                })?;
        }

        Ok(StreamableVideo {
            path: MoviePath::Remote(movie_path),
            formats: vec![StreamableVideoFormat::HLS(HLS::default())],
            movie: converted.movie,
        })
    }
}
