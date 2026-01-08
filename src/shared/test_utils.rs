use anyhow::Result;
use std::{ops::DerefMut, path::Path};

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use std::{
    net::Ipv4Addr,
    ops::Deref,
    path::PathBuf,
    sync::{Arc, Once},
};

use chrono::DateTime;

use crate::{
    app::{AppBuilder, AppState},
    models::users::User,
    services::{
        movie_manager::MovieManager,
        movies::SimpleMovieService,
        torrent::{DownloadManager, SimpleTorrentService},
    },
};

static INIT: Once = Once::new();

pub async fn setup_test_app() -> Result<(String, AppState)> {
    INIT.call_once(|| {
        let instrumentation = crate::shared::logging::Instrumentation {
            verbose: 1,
            ..Default::default()
        };
        let _ = instrumentation.setup();
    });

    // setup throughaway database connection
    let db_url = "sqlite::memory:".to_string();
    let db_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON;")
                    .execute(conn)
                    .await
                    .expect("Failed to enable foreign keys");
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    let movie_service = SimpleMovieService::new(db_pool.clone());
    let movie_manager = MovieManager::initialize(Arc::new(movie_service.clone()), &db_pool).await;
    let download_manager = DownloadManager::new().await;
    let torrent_service = SimpleTorrentService::new(download_manager.clone(), &db_pool);

    let streaming_service =
        crate::services::streaming::SimpleStreamingService::new(movie_manager.clone());
    let metrics_service = crate::services::metrics::SimpleStateReporter::new(
        movie_manager.clone(),
        download_manager.clone(),
    );
    let converter = crate::services::converter::FFmpegConverter;

    let state = AppState::new(
        db_pool.clone(),
        movie_manager,
        download_manager,
        converter,
        Arc::new(torrent_service),
        Arc::new(streaming_service),
        Arc::new(metrics_service),
    )
    .await;

    let app = AppBuilder::new()
        .address(Ipv4Addr::new(127, 0, 0, 1))
        .port(None)
        .tls(false)
        .prod(false)
        .build()
        .await
        .expect("Failed to build test app")
        .with_state(Arc::new(state.clone()));

    let test_valid_user = test_user();

    let _user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, display_name, is_admin, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(&test_valid_user.username)        // Individual field
    .bind(&test_valid_user.email)           // Individual field
    .bind(&test_valid_user.password_hash)
    .bind(&test_valid_user.display_name)    // Individual field
    .bind(test_valid_user.is_admin)         // Individual field
    .bind(test_valid_user.created_at)       // Individual field
    .bind(test_valid_user.updated_at)       // Individual field
    .fetch_one(&db_pool)
    .await?;

    let address = app.address().expect("Failed to get app address");

    tokio::spawn(async move {
        app.run(db_pool).await.expect("Failed to run test app");
    });

    let address = format!("http://{}", address);
    Ok((address, state.clone()))
}

pub fn test_user() -> User {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password("hunter42".as_bytes(), &salt)
        .unwrap()
        .to_string();

    let some_date = DateTime::from_timestamp(1415923200, 0).expect("Failed to create test date");

    User {
        id: 1,
        username: "ferris".to_string(),
        email: "ferris@gmail.com".to_string(),
        password_hash,
        display_name: None,
        is_admin: false,
        created_at: some_date,
        updated_at: some_date,
    }
}

#[derive(Debug, Clone)]
pub struct TempDir {
    inner: PathBuf,
}

impl TempDir {
    pub fn create() -> Result<Self, std::io::Error> {
        let dir = std::env::temp_dir().join(format!("tempdir_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self { inner: dir })
    }

    pub fn path(&self) -> &PathBuf {
        &self.inner
    }
}

impl Deref for TempDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TempDir {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.inner.exists() {
            // should we really unwrap here?
            std::fs::remove_dir_all(&self.inner).expect("Failed to remove temp dir");
        }
    }
}

#[derive(Debug, Clone)]
pub struct TempFile {
    inner: PathBuf,
}

impl TempFile {
    pub fn create(contents: &str, path: &Path) -> Result<Self, std::io::Error> {
        let file_path = path.join(format!("tempfile_{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&file_path, contents)?;
        Ok(Self { inner: file_path })
    }

    pub fn create_named(name: &str, contents: &str, path: PathBuf) -> Result<Self, std::io::Error> {
        let file_path = path.join(name);
        std::fs::write(&file_path, contents)?;
        Ok(Self { inner: file_path })
    }

    pub fn path(&self) -> &PathBuf {
        &self.inner
    }
}

impl Deref for TempFile {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.inner.exists() {
            std::fs::remove_file(&self.inner).expect("Failed to remove temp file");
        }
    }
}

pub fn test_movie() -> crate::models::movie::Movie {
    crate::models::movie::Movie {
        id: i64::MAX,
        imdb_id: "tt9999999".to_string(),
        title: "Test Movie".to_string(),
        production_company: "Test Productions".to_string(),
        release_year: 2024,
        genre: "Drama".to_string(),
        state: crate::models::movie::MovieState::Available,
        created_at: Some(time::OffsetDateTime::from_unix_timestamp(1_615_000_000).unwrap()),
        updated_at: Some(time::OffsetDateTime::from_unix_timestamp(1_615_000_000).unwrap()),
    }
}

pub async fn setup_test_db() -> Result<sqlx::SqlitePool> {
    let db_url = "sqlite::memory:".to_string();
    let db_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON;")
                    .execute(conn)
                    .await
                    .expect("Failed to enable foreign keys");
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    Ok(db_pool)
}
