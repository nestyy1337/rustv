use futures::StreamExt;
use futures::stream::FuturesUnordered;
use quick_xml::de::from_str;
use reqwest::redirect::Policy;
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use crate::{
    models::movie::Movie,
    services::{
        converter::{FFmpegConverter, RawVideoFormat, VideoCodec},
        movie_manager::{MovieManager, MovieStoreage},
    },
    shared::{
        config::SETTINGS,
        error::{Error, GenericSnafu, LibrqbSnafu},
    },
};
#[cfg(test)]
use librqbit::TorrentStatsState;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, SessionOptions, TorrentStats,
};

#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<String, reqwest::Error>;
}

pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .redirect(Policy::limited(10))
            .build()
            .expect("Failed to build reqwest client");
        Self { client }
    }
}

#[async_trait::async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<String, reqwest::Error> {
        self.client.get(url).query(query).send().await?.text().await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TorrentServiceError {
    #[error("download error: {0}")]
    ReqwestError(reqwest::Error),
    #[error("torrent download error: {0}")]
    TorrentDownloadError(String),
    #[error("no valid video file downloaded")]
    NoValidVideoFile,
}

#[derive(Clone)]
pub struct SimpleTorrentService<T: TorrentSession + 'static> {
    download_manager: DownloadManager<T>,
    pool: Pool<Sqlite>,
    client: Arc<dyn HttpClient>,
}

impl<T: TorrentSession + 'static> SimpleTorrentService<T> {
    pub fn new_with_client(
        download_manager: DownloadManager<T>,
        pool: &Pool<Sqlite>,
        client: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            download_manager,
            pool: pool.clone(),
            client,
        }
    }
}

#[derive(Debug)]
pub enum MovieIdentifier<'a> {
    TmdbId(&'a str),
    Title(&'a str),
}

impl std::fmt::Display for MovieIdentifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MovieIdentifier::TmdbId(id) => write!(f, "{}", id),
            MovieIdentifier::Title(title) => write!(f, "{}", title),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadedMovie {
    pub title: String,
    pub path: PathBuf,
    pub codec: VideoCodec,
    pub format: RawVideoFormat,
}

const TRUSTED_UPLOADERS: [&str; 7] = [
    "EZTV", "licdom", "CptKC", "TvTeam", "Retr022", "YIFI", "UserHEVC",
];

#[async_trait::async_trait]
pub trait TorrentService {
    async fn search_movie(&self, movie: &MovieIdentifier)
    -> Result<Vec<Item<New>>, reqwest::Error>;
    async fn search_raw_jackett(
        &self,
        query: &str,
        indexer: &str,
    ) -> Result<Vec<Item<New>>, reqwest::Error>;
}

pub struct MagnetURIUnchecked(pub String);

impl Deref for MagnetURIUnchecked {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default)]
pub struct MagnetURI(String);

impl AsRef<str> for MagnetURI {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MagnetURI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for MagnetURI {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MagnetURI {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<MagnetURIUnchecked> for MagnetURI {
    type Error = Error;
    fn try_from(unchecked: MagnetURIUnchecked) -> Result<Self, Error> {
        tracing::info!(uri = %unchecked.0, "Validating magnet URI");

        if !unchecked.starts_with("magnet:") {
            return GenericSnafu {
                reason: "Invalid magnet URI: must start with 'magnet:'",
            }
            .fail();
        }

        Ok(MagnetURI(unchecked.0))
    }
}

#[async_trait::async_trait]
impl TorrentService for SimpleTorrentService<TorrentSessionManager> {
    #[tracing::instrument(skip(self))]
    async fn search_raw_jackett(
        &self,
        query: &str,
        indexer: &str,
    ) -> Result<Vec<Item<New>>, reqwest::Error> {
        let url = format!(
            "{}/api/v2.0/indexers/{}/results/torznab/api",
            "http://localhost:9117", indexer
        );

        let query = query.to_string();
        tracing::info!(%query, %indexer, "Searching Jackett with raw query");

        let response = self
            .client
            .get(
                &url,
                &[
                    ("apikey", &SETTINGS.application.apikeys.jackett),
                    ("t", "search"),
                    ("q", &query),
                    ("cat", "7000"),
                ],
            )
            .await?;
        let rss: Rss<New> = from_str(&response).unwrap();

        Ok(rss.channel.items)
    }

    #[tracing::instrument(skip(self))]
    async fn search_movie(
        &self,
        movie: &MovieIdentifier,
    ) -> Result<Vec<Item<New>>, reqwest::Error> {
        let url = format!(
            "{}/api/v2.0/indexers/{}/results/torznab/api",
            "http://localhost:9117", "torrentgalaxyclone"
        );

        let query = movie.to_string();
        tracing::info!(%query, "Searching Jackett for movie");

        let response = self
            .client
            .get(
                &url,
                &[
                    ("apikey", &SETTINGS.application.apikeys.jackett),
                    ("t", "search"),
                    ("q", &query),
                    ("cat", "2000"),
                ],
            )
            .await?;
        let rss: Rss<New> = from_str(&response).unwrap();

        let mut items = rss.channel.items;
        tracing::info!(found = %items.len(), "Found items from Jackett search");
        items.retain(|item| {
            if let Some(uploader) = &item.uploader {
                TRUSTED_UPLOADERS.contains(&uploader.0.as_str())
            } else {
                false
            }
        });
        items.truncate(5);
        // items.sort_by(|a, b| b.seeders().unwrap_or(0).cmp(&a.seeders().unwrap_or(0)));
        items.sort_by_key(|item| std::cmp::Reverse(item.seeders().unwrap_or(0)));
        tracing::info!(filtered = %items.len(), "Filtered items from Jackett search");
        Ok(items)
    }
}

impl std::fmt::Debug for SimpleTorrentService<TorrentSessionManager> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleTorrentService").finish()
    }
}

impl SimpleTorrentService<TorrentSessionManager> {
    pub fn new(
        download_manager: DownloadManager<TorrentSessionManager>,
        pool: &Pool<Sqlite>,
    ) -> Self {
        Self {
            download_manager,
            pool: pool.clone(),
            client: Arc::new(ReqwestHttpClient::new()),
        }
    }
}

use serde::{Deserialize, Serialize, ser::SerializeStruct};

#[derive(Debug, Deserialize)]
struct Rss<ItemState: Default> {
    channel: Channel<ItemState>,
}

#[derive(Debug, Deserialize, Clone)]
struct Channel<ItemState> {
    #[serde(rename = "item", default)]
    items: Vec<Item<ItemState>>,
}

#[derive(Debug, Clone)]
pub struct Uploader(pub String);

impl<'de> Deserialize<'de> for Uploader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let uploader_name = s
            .strip_prefix("Uploader: ")
            .ok_or(serde::de::Error::custom(
                "Uploader string does not start with 'Uploader: '",
            ))?
            .to_string();
        Ok(Uploader(uploader_name))
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Downloaded;
#[derive(Debug, Deserialize, Clone, Default)]
pub struct New;
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Failed;

#[derive(Debug, Deserialize, Clone)]
pub struct Item<ItemState> {
    pub title: String,
    pub link: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(rename = "description", default)]
    pub uploader: Option<Uploader>,
    #[serde(rename = "pubDate", default)]
    pub pub_date: Option<String>,
    #[serde(rename = "attr", default)]
    pub attributes: Vec<TorznabAttr>,
    #[serde(skip)]
    marker: PhantomData<ItemState>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TorznabAttr {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@value")]
    pub value: String,
}

impl Item<New> {
    pub fn seeders(&self) -> Option<u32> {
        self.attributes
            .iter()
            .find(|attr| attr.name == "seeders")
            .and_then(|attr| attr.value.parse().ok())
    }

    pub fn peers(&self) -> Option<u32> {
        self.attributes
            .iter()
            .find(|attr| attr.name == "peers")
            .and_then(|attr| attr.value.parse().ok())
    }
}

impl<S> Item<S> {
    pub fn size_gb(&self) -> String {
        format!(
            "{:.1}",
            self.size
                .map(|s| s as f64 / 1024.0 / 1024.0 / 1024.0)
                .unwrap_or(0.0)
        )
    }
}

use snafu::ResultExt;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct ProcessingStatus {
    pub elapsed: String,
    pub speed: String,
    pub progress: String,
}

impl Default for ProcessingStatus {
    fn default() -> Self {
        Self {
            elapsed: "00:00:00".to_string(),
            speed: "0x".to_string(),
            progress: "0%".to_string(),
        }
    }
}

impl ProcessingStatus {
    pub fn from_stderr(stderr: &str) -> Option<Self> {
        let mut elapsed: Option<String> = None;
        let mut speed: Option<String> = None;
        let mut progress: Option<String> = None;

        let parts = stderr.split_whitespace();
        let mut prev_name = String::new();
        for part in parts {
            if part.starts_with("time=")
                && let Some(time) = part.split('=').nth(1)
            {
                elapsed = Some(time.to_string());
            }
            if part.starts_with("speed=")
                && let Some(spd) = part.split('=').nth(1)
            {
                speed = Some(spd.to_string());
            }
            if prev_name == "frame=" {
                progress = Some(part.to_string());
            }
            prev_name = part.to_string();
        }

        if elapsed.is_none() || speed.is_none() || progress.is_none() {
            return None;
        }

        Some(Self {
            elapsed: elapsed.unwrap(),
            speed: speed.unwrap(),
            progress: progress.unwrap(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ActiveProcessing {
    pub status: ProcessingStatus,
}

impl From<ActiveDownload> for ActiveProcessing {
    fn from(_download: ActiveDownload) -> Self {
        Self {
            status: ProcessingStatus::default(),
        }
    }
}

impl ActiveProcessing {
    pub fn new(_movie: Movie) -> Self {
        Self {
            status: ProcessingStatus::default(),
        }
    }
}

pub trait TorrentHandle {
    fn torrent_stats(&self) -> TorrentStats;
    fn torrent_id(&self) -> usize;
    fn torrent_name(&self) -> Option<String>;
}

impl TorrentHandle for Arc<ManagedTorrent> {
    fn torrent_stats(&self) -> TorrentStats {
        self.stats()
    }
    fn torrent_id(&self) -> usize {
        self.id()
    }
    fn torrent_name(&self) -> Option<String> {
        self.name()
    }
}

#[async_trait::async_trait]
pub trait TorrentSession: Send + Sync {
    async fn add_torrent(&self, magnet: &str, movie: &Movie)
    -> Result<ManagedTorrentHandle, Error>;
    async fn delete(&self, id: TorrentSearchType) -> Result<(), Error>;
    async fn get_all_downloads(&self) -> Vec<(TorrentID, ActiveDownload)>;
    async fn get_download_keys(&self) -> Vec<TorrentID>;
    async fn get_download(&self, id: &TorrentID) -> Option<ActiveDownload>;
    async fn get_download_by_search(&self, search: &TorrentSearchType) -> Option<ActiveDownload>;
    async fn update_download_status(&self, id: &TorrentID, status: DownloadStatus);
    async fn remove_download(&self, id: &TorrentID) -> Option<ActiveDownload>;
    async fn get_active_downloads(&self) -> Vec<(TorrentID, ActiveDownload)>;
}

#[async_trait::async_trait]
impl TorrentSession for TorrentSessionManager {
    async fn add_torrent(
        &self,
        magnet: &str,
        movie: &Movie,
    ) -> Result<ManagedTorrentHandle, Error> {
        let handle = match self
            .session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions {
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .context(LibrqbSnafu {})?
        {
            AddTorrentResponse::Added(_, handle) => handle,
            AddTorrentResponse::AlreadyManaged(_, _) => {
                tracing::warn!(magnet = %magnet, "Torrent already added to session");
                return Err(GenericSnafu {
                    reason: "Torrent already managed".to_string(),
                }
                .build())?;
            }
            _ => {
                tracing::error!(magnet = %magnet, "Failed to add torrent to session");
                return Err(GenericSnafu {
                    reason: "Failed to add torrent".to_string(),
                }
                .build())?;
            }
        };

        let active_download = ActiveDownload {
            handle: ManagedTorrentHandle {
                inner: Arc::new(handle.clone()),
            },
            status: DownloadStatus::Downloading { progress: 0.0 },
            movie: movie.clone(),
        };

        let torrent_id = TorrentID::new(handle.id(), movie.id as usize, movie.imdb_id.clone());
        self.downloads
            .write()
            .await
            .insert(torrent_id, active_download);

        tracing::error!("added torrent to session");

        Ok(ManagedTorrentHandle {
            inner: Arc::new(handle),
        })
    }

    async fn delete(&self, torrent_id: TorrentSearchType) -> Result<(), Error> {
        let id = match torrent_id {
            TorrentSearchType::ByID(id) => id,
            TorrentSearchType::ByMovieID(movie_id) => {
                let downloads = self.downloads.read().await;
                let entry = downloads
                    .values()
                    .find(|download| download.movie.id == movie_id)
                    .ok_or_else(|| {
                        GenericSnafu {
                            reason: format!("No active download found for movie ID {}", movie_id),
                        }
                        .build()
                    })?;
                entry.handle.id()
            }
            _ => {
                return GenericSnafu {
                    reason: "Unsupported torrent search type for deletion".to_string(),
                }
                .fail();
            }
        };

        self.session
            .delete(librqbit::api::TorrentIdOrHash::Id(id), true)
            .await
            .context(LibrqbSnafu {})?;
        self.downloads.write().await.retain(|key, _| key.id != id);
        Ok(())
    }

    async fn get_all_downloads(&self) -> Vec<(TorrentID, ActiveDownload)> {
        self.downloads
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    async fn get_download_keys(&self) -> Vec<TorrentID> {
        self.downloads.read().await.keys().cloned().collect()
    }

    async fn get_download(&self, id: &TorrentID) -> Option<ActiveDownload> {
        self.downloads.read().await.get(id).cloned()
    }

    async fn get_download_by_search(&self, search: &TorrentSearchType) -> Option<ActiveDownload> {
        self.downloads
            .read()
            .await
            .iter()
            .find_map(|(key, entry)| match search {
                TorrentSearchType::ByTmdbId(id) if &key.tmdb_id == id => Some(entry.clone()),
                TorrentSearchType::ByMovieID(movie_id) if key.movie_id as i64 == *movie_id => {
                    Some(entry.clone())
                }
                TorrentSearchType::ByID(id) if key.id == *id => Some(entry.clone()),
                _ => None,
            })
    }

    async fn update_download_status(&self, id: &TorrentID, status: DownloadStatus) {
        if let Some(download) = self.downloads.write().await.get_mut(id) {
            download.status = status;
        }
    }

    async fn remove_download(&self, id: &TorrentID) -> Option<ActiveDownload> {
        self.downloads.write().await.remove(id)
    }

    async fn get_active_downloads(&self) -> Vec<(TorrentID, ActiveDownload)> {
        self.downloads
            .read()
            .await
            .iter()
            .filter_map(|(key, entry)| {
                if let DownloadStatus::Downloading { .. } = &entry.status {
                    Some((key.clone(), entry.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct TorrentID {
    id: usize,
    movie_id: usize,
    pub tmdb_id: String,
}

impl std::fmt::Display for TorrentID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TorrentID(id: {}, movie_id: {}, tmdb_id: {})",
            self.id, self.movie_id, self.tmdb_id
        )
    }
}

impl TorrentID {
    pub fn new(id: usize, movie_id: usize, tmdb_id: String) -> Self {
        Self {
            id,
            movie_id,
            tmdb_id,
        }
    }
}

#[derive(Clone, Debug)]
pub enum TorrentSearchType {
    ByMovieID(i64),
    ByID(usize),
    ByTmdbId(String),
}

#[derive(Clone)]
pub struct TorrentSessionManager {
    session: Arc<librqbit::Session>,
    downloads: Arc<RwLock<HashMap<TorrentID, ActiveDownload>>>,
}

impl TorrentSessionManager {
    pub async fn new() -> Result<Self, Error> {
        let session = librqbit::Session::new_with_opts(
            PathBuf::from("downloads"),
            SessionOptions {
                disable_dht_persistence: true,
                fastresume: true,
                ..Default::default()
            },
        )
        .await
        .context(LibrqbSnafu {})?;

        Ok(Self {
            session,
            downloads: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[derive(Clone)]
pub struct ManagedTorrentHandle {
    inner: Arc<dyn TorrentHandle + Send + Sync>,
}

impl ManagedTorrentHandle {
    pub fn stats(&self) -> TorrentStats {
        self.inner.torrent_stats()
    }

    pub fn id(&self) -> usize {
        self.inner.torrent_id()
    }

    pub fn name(&self) -> Option<String> {
        self.inner.torrent_name()
    }
}

impl Deref for ManagedTorrentHandle {
    type Target = dyn TorrentHandle + Send + Sync;
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

#[derive(Clone)]
pub struct ActiveDownload {
    pub handle: ManagedTorrentHandle,
    pub status: DownloadStatus,
    pub movie: Movie,
}

impl ActiveDownload {
    pub fn is_completed(&self) -> bool {
        matches!(self.status, DownloadStatus::Completed(_))
    }
    pub fn is_downloading(&self) -> bool {
        matches!(self.status, DownloadStatus::Downloading { .. })
    }
    pub fn is_failed(&self) -> bool {
        matches!(self.status, DownloadStatus::Failed(_))
            || self.handle.torrent_stats().error.is_some()
    }

    pub fn status_text(&self) -> &str {
        match self.status {
            DownloadStatus::Initializing => "Initializing",
            DownloadStatus::Downloading { .. } => "Downloading",
            DownloadStatus::Completed(_) => "Completed",
            DownloadStatus::Failed(..) => "Failed",
        }
    }

    pub fn downloaded(&self) -> String {
        match &self.status {
            DownloadStatus::Downloading { progress } => {
                format!("{:.1}/{}", progress, 100.0)
            }
            DownloadStatus::Completed(_) => {
                format!("{}/{}", 100.0, 100.0)
            }
            _ => "0/100".to_string(),
        }
    }
}

impl Serialize for ActiveDownload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ActiveDownload", 3)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("movie", &self.movie)?;
        state.end()
    }
}

impl std::fmt::Debug for ActiveDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveDownload")
            .field("handle_id", &self.handle.id())
            .field("status", &self.status)
            .field("movie_title", &self.movie.title)
            .finish()
    }
}

pub type TmdbId = String;

#[derive(Clone)]
pub struct DownloadManager<T: TorrentSession + 'static> {
    pub session: Arc<T>,
    pub failed_downloads: Arc<RwLock<Vec<(TmdbId, String)>>>,
    pub processing: Arc<RwLock<HashMap<String, ActiveProcessing>>>,
}

impl<T: TorrentSession> DownloadManager<T> {
    pub fn new_with_session(session: Arc<T>) -> Self {
        Self {
            session,
            failed_downloads: Arc::new(RwLock::new(Vec::new())),
            processing: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_download(
        &self,
        tmdb_id: &str,
        magnet_uri: MagnetURI,
        movie: Movie,
    ) -> Result<(), Error> {
        match self.session.add_torrent(&magnet_uri, &movie).await {
            Ok(handle) => handle,
            Err(e) => {
                tracing::error!(tmdb_id = %tmdb_id, error = %e, "Failed to add torrent");
                return Err(e);
            }
        };
        tracing::info!(tmdb_id = %tmdb_id, "Started download for movie");

        Ok(())
    }
    pub async fn stop_download(&self, tmdb_id: TorrentSearchType) -> Result<(), Error> {
        let download = self.session.delete(tmdb_id.clone()).await;
        match download {
            Ok(_) => {
                tracing::info!(tmdb_id = ?tmdb_id, "Stopped and removed download");
                Ok(())
            }
            Err(e) => {
                tracing::error!(tmdb_id = ?tmdb_id, error = %e, "Failed to stop download");
                Err(e)
            }
        }
    }
}

impl std::fmt::Debug for DownloadManager<TorrentSessionManager> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadManager")
            .field("active_downloads", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl DownloadManager<TorrentSessionManager> {
    pub async fn new() -> Self {
        // cringe
        #[cfg(feature = "integration-tests")]
        {
            tracing::info!("Clearing DHT cache in test environment");
            if let Some(home) = std::env::var_os("HOME") {
                let dht_cache = PathBuf::from(home).join(".cache/com.rqbit.dht/dht.json");
                let _ = tokio::fs::remove_file(&dht_cache);
                tracing::info!(dht_cache = ?dht_cache, "Attempted to remove DHT cache");
            }
        }

        Self {
            failed_downloads: Arc::new(RwLock::new(Vec::new())),
            processing: Arc::new(RwLock::new(HashMap::new())),
            session: Arc::new(TorrentSessionManager::new().await.unwrap()),
        }
    }

    pub async fn monitor_downloads<T: MovieStoreage + Send + Sync + Clone + 'static>(
        self,
        movie_manager: MovieManager<T>,
    ) {
        let processing = self.processing.clone();
        let converter = FFmpegConverter;

        tokio::spawn(async move {
            let mut futures = FuturesUnordered::new();
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    tracing::debug!("Checking active downloads...");

                    let keys = self.session.get_download_keys().await;

                    for tmdb_id in keys {
                        let entry_data = self.session.get_download(&tmdb_id).await;

                        if let Some(entry) = entry_data {
                            let stats = entry.handle.stats();

                            let is_complete =
                                stats.progress_bytes == stats.total_bytes && stats.total_bytes > 0;
                            let has_error = stats.error.is_some();

                            if has_error {
                                self.stop_download(TorrentSearchType::ByMovieID(entry.movie.id)).await.ok();
                                tracing::info!(
                                    tmdb_id = ?tmdb_id,
                                    movie_id = entry.movie.id,
                                    movie_title = %entry.movie.title,
                                    "Removed failed download from active downloads"
                                );
                                self.failed_downloads
                                    .write()
                                    .await
                                    .push((tmdb_id.tmdb_id, stats.error.unwrap()));
                                continue;
                            }

                            let new_status = DownloadStatus::from(&stats, &entry.movie);
                            self.session.update_download_status(&tmdb_id, new_status).await;

                            if is_complete {
                                tracing::info!(
                                    tmdb_id = ?tmdb_id,
                                    movie_id = entry.movie.id,
                                    movie_title = %entry.movie.title,
                                    "Download complete, starting processing pipeline"
                                );

                                let movie = entry.movie.clone();
                                let active_download = entry.clone();
                                let movie_manager = movie_manager.clone();
                                let converter = converter.clone();
                                let processing = processing.clone();
                                self.session.remove_download(&tmdb_id).await;
                                futures.push(tokio::spawn(async move {
                                    let result = movie_manager
                                        .complete_download(active_download, processing, &converter)
                                        .await;
                                    (tmdb_id, movie.id, result)
                                }));

                            }
                        }
                    }
                }
                    Some(result) = futures.next() => {
                          match result {
                              Ok((_tmdb_id, id, Ok(_))) => {
                                  tracing::info!(movie_id = id, "Successfully completed download pipeline");
                              }
                              Ok((tmdb_id, id, Err(e))) => {
                                  tracing::error!(movie_id = id, error = %e, "Failed to complete download pipeline");
                                  self.stop_download(TorrentSearchType::ByTmdbId(tmdb_id.tmdb_id.clone())).await.ok();
                                  self.failed_downloads.write().await.push((tmdb_id.tmdb_id.clone(), e.to_string()));
                              }
                              Err(e) => {
                                  tracing::error!(error = %e, "Conversion task panicked");
                              }
                          }
                      }

                }
            }
        });
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadProgress {
    pub title: String,
    pub torrent_id: usize,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub status: DownloadStatus,
}

impl From<TorrentStats> for DownloadProgress {
    fn from(stats: TorrentStats) -> Self {
        Self {
            title: String::new(),
            torrent_id: 0,
            downloaded_bytes: stats.progress_bytes,
            total_bytes: stats.total_bytes,
            percentage: if stats.total_bytes > 0 {
                stats.progress_bytes as f32 / stats.total_bytes as f32 * 100.0
            } else {
                0.0
            },
            status: DownloadStatus::Downloading { progress: 0.0 },
        }
    }
}

impl DownloadProgress {
    pub fn new(handle: Arc<ManagedTorrent>) -> Self {
        Self {
            title: handle.name().unwrap_or_default(),
            torrent_id: handle.id(),
            downloaded_bytes: 0,
            total_bytes: 0,
            percentage: 0.0,
            status: DownloadStatus::Initializing,
        }
    }

    pub fn update(&mut self, stats: &TorrentStats) {
        self.downloaded_bytes = stats.progress_bytes;
        self.total_bytes = stats.total_bytes;
        if self.total_bytes > 0 {
            self.percentage = self.downloaded_bytes as f32 / self.total_bytes as f32 * 100.0;
        } else {
            self.percentage = 0.0;
        }
    }

    pub fn downloaded_bytes_progress(&self) -> String {
        // x out of y MB like x / y MB/GB
        let mut total_downloaded = 0u64;
        let mut total_size = 0u64;
        total_downloaded += self.downloaded_bytes;
        total_size += self.total_bytes;
        format!(
            "{} / {}",
            human_bytes::human_bytes(total_downloaded as f64),
            human_bytes::human_bytes(total_size as f64)
        )
    }
    pub fn downloaded_gb(&self) -> f64 {
        self.downloaded_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }

    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0
    }

    pub fn downloaded_gb_str(&self) -> String {
        format!("{:.2} GB", self.downloaded_gb())
    }

    pub fn total_gb_str(&self) -> String {
        format!("{:.2} GB", self.total_gb())
    }

    pub fn percentage_rounded(&self) -> f32 {
        (self.percentage * 10.0).round() / 10.0
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self.status, DownloadStatus::Downloading { .. })
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, DownloadStatus::Completed(_))
    }

    pub fn status_text(&self) -> &str {
        match &self.status {
            DownloadStatus::Failed(..) => "Failed",
            DownloadStatus::Completed(_) => "Completed",
            DownloadStatus::Downloading { .. } => "Downloading",
            DownloadStatus::Initializing => "Initializing",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum DownloadStatus {
    Initializing,
    Downloading { progress: f64 },
    Completed(DownloadedMovie),
    Failed(String),
}

impl DownloadStatus {
    pub fn from(stats: &TorrentStats, movie: &Movie) -> Self {
        let is_complete = stats.progress_bytes == stats.total_bytes && stats.total_bytes > 0;
        let has_error = stats.error.is_some();

        if has_error {
            DownloadStatus::Failed(stats.error.clone().unwrap())
        } else if is_complete {
            let torrent_download_path =
                PathBuf::from("downloads").join(movie.title.replace(" ", "_"));

            DownloadStatus::Completed(DownloadedMovie {
                title: movie.title.clone(),
                path: torrent_download_path,
                codec: VideoCodec::H264,
                format: RawVideoFormat::MP4,
            })
        } else {
            DownloadStatus::Downloading {
                progress: if stats.total_bytes > 0 {
                    stats.progress_bytes as f64 / stats.total_bytes as f64 * 100.0
                } else {
                    0.0
                },
            }
        }
    }
}

impl Serialize for MovieIdentifier<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_magnet_uri_validation() {
        let valid_magnet = MagnetURIUnchecked("magnet:?xt=urn:btih:abcdef1234567890".to_string());
        let invalid_magnet = MagnetURIUnchecked("http://example.com/torrent".to_string());

        assert!(MagnetURI::try_from(valid_magnet).is_ok());
        assert!(MagnetURI::try_from(invalid_magnet).is_err());
    }

    #[test]
    fn test_magnet_uri_validation_failure() {
        let invalid = MagnetURIUnchecked("http://example.com".to_string());
        assert!(MagnetURI::try_from(invalid).is_err());
    }

    #[test]
    fn test_magnet_uri_validation_empty() {
        let empty = MagnetURIUnchecked("".to_string());
        assert!(MagnetURI::try_from(empty).is_err());
    }

    #[test]
    fn test_item_seeders_parsing() {
        let item = Item::<New> {
            title: "Test".into(),
            link: "link".into(),
            size: None,
            uploader: None,
            pub_date: None,
            attributes: vec![TorznabAttr {
                name: "seeders".into(),
                value: "42".into(),
            }],
            marker: PhantomData,
        };
        assert_eq!(item.seeders(), Some(42));
    }

    #[test]
    fn test_item_seeders_missing() {
        let item = Item::<New> {
            title: "Test".into(),
            link: "link".into(),
            size: None,
            uploader: None,
            pub_date: None,
            attributes: vec![],
            marker: PhantomData,
        };
        assert_eq!(item.seeders(), None);
    }

    #[test]
    fn test_item_size_gb() {
        let item = Item::<New> {
            title: "Test".into(),
            link: "link".into(),
            size: Some(5_368_709_120),
            uploader: None,
            pub_date: None,
            attributes: vec![],
            marker: PhantomData,
        };
        assert_eq!(item.size_gb(), "5.0");
    }

    #[test]
    fn test_uploader_deserialization() {
        let json = r#""Uploader: EZTV""#;
        let uploader: Uploader = serde_json::from_str(json).unwrap();
        assert_eq!(uploader.0, "EZTV");
    }

    #[test]
    fn test_uploader_deserialization_invalid() {
        let json = r#""EZTV""#;
        assert!(serde_json::from_str::<Uploader>(json).is_err());
    }

    struct MockHttpClient {
        response: String,
    }

    #[async_trait::async_trait]
    impl HttpClient for MockHttpClient {
        async fn get(&self, _url: &str, _query: &[(&str, &str)]) -> Result<String, reqwest::Error> {
            Ok(self.response.clone())
        }
    }

    struct MockTorrentHandle {
        id: usize,
        progress_bytes: u64,
        total_bytes: u64,
        error: Option<String>,
    }

    impl MockTorrentHandle {
        fn new(id: usize, progress_bytes: u64, total_bytes: u64, error: Option<String>) -> Self {
            Self {
                id,
                progress_bytes,
                total_bytes,
                error,
            }
        }
    }

    impl TorrentHandle for MockTorrentHandle {
        fn torrent_stats(&self) -> TorrentStats {
            TorrentStats {
                state: TorrentStatsState::Initializing,
                progress_bytes: self.progress_bytes,
                total_bytes: self.total_bytes,
                error: self.error.clone(),
                uploaded_bytes: 0,
                file_progress: vec![],
                live: None,
                finished: self.progress_bytes == self.total_bytes && self.total_bytes > 0,
            }
        }

        fn torrent_id(&self) -> usize {
            self.id
        }

        fn torrent_name(&self) -> Option<String> {
            Some("Mock Torrent".to_string())
        }
    }

    #[derive(Clone)]
    struct MockTorrentSession {
        downloads: Arc<RwLock<HashMap<TorrentID, ActiveDownload>>>,
    }

    impl MockTorrentSession {
        fn new() -> Self {
            Self {
                downloads: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl TorrentSession for MockTorrentSession {
        async fn add_torrent(
            &self,
            _magnet: &str,
            movie: &Movie,
        ) -> Result<ManagedTorrentHandle, Error> {
            let mock_handle = MockTorrentHandle::new(1, 0, 100, None);
            let active = ActiveDownload {
                handle: ManagedTorrentHandle {
                    inner: Arc::new(mock_handle),
                },
                status: DownloadStatus::Initializing,
                movie: movie.clone(),
            };

            let id = TorrentID::new(1, movie.id as usize, movie.imdb_id.clone());
            self.downloads.write().await.insert(id, active);

            Ok(ManagedTorrentHandle {
                inner: Arc::new(MockTorrentHandle::new(1, 0, 100, None)),
            })
        }

        async fn delete(&self, search: TorrentSearchType) -> Result<(), Error> {
            let downloads = self.downloads.read().await;
            let key_to_remove = downloads.iter().find_map(|(key, download)| match &search {
                TorrentSearchType::ByMovieID(movie_id) if download.movie.id == *movie_id => {
                    Some(key.clone())
                }
                TorrentSearchType::ByID(id) if key.id == *id => Some(key.clone()),
                TorrentSearchType::ByTmdbId(tmdb) if &key.tmdb_id == tmdb => Some(key.clone()),
                _ => None,
            });
            drop(downloads);

            if let Some(key) = key_to_remove {
                self.downloads.write().await.remove(&key);
            }
            Ok(())
        }

        async fn get_all_downloads(&self) -> Vec<(TorrentID, ActiveDownload)> {
            self.downloads
                .read()
                .await
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }

        async fn get_download_keys(&self) -> Vec<TorrentID> {
            self.downloads.read().await.keys().cloned().collect()
        }

        async fn get_download(&self, id: &TorrentID) -> Option<ActiveDownload> {
            self.downloads.read().await.get(id).cloned()
        }

        async fn get_download_by_search(
            &self,
            search: &TorrentSearchType,
        ) -> Option<ActiveDownload> {
            self.downloads
                .read()
                .await
                .iter()
                .find_map(|(key, entry)| match search {
                    TorrentSearchType::ByTmdbId(id) if &key.tmdb_id == id => Some(entry.clone()),
                    TorrentSearchType::ByMovieID(movie_id) if key.movie_id as i64 == *movie_id => {
                        Some(entry.clone())
                    }
                    TorrentSearchType::ByID(id) if key.id == *id => Some(entry.clone()),
                    _ => None,
                })
        }

        async fn update_download_status(&self, id: &TorrentID, status: DownloadStatus) {
            if let Some(download) = self.downloads.write().await.get_mut(id) {
                download.status = status;
            }
        }

        async fn remove_download(&self, id: &TorrentID) -> Option<ActiveDownload> {
            self.downloads.write().await.remove(id)
        }

        async fn get_active_downloads(&self) -> Vec<(TorrentID, ActiveDownload)> {
            self.downloads
                .read()
                .await
                .iter()
                .filter_map(|(key, entry)| {
                    if let DownloadStatus::Downloading { .. } = &entry.status {
                        Some((key.clone(), entry.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        }
    }

    fn test_movie() -> Movie {
        Movie {
            id: 1,
            imdb_id: "tt12345".to_string(),
            title: "Test Movie".to_string(),
            production_company: "Test Studios".to_string(),
            release_year: 2024,
            genre: "Action".to_string(),
            state: crate::models::movie::MovieState::Available,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_download_status_from_stats_downloading() {
        let handle = MockTorrentHandle::new(1, 50, 100, None);
        let stats = handle.torrent_stats();
        let movie = test_movie();
        let status = DownloadStatus::from(&stats, &movie);

        match status {
            DownloadStatus::Downloading { progress } => {
                assert_eq!(progress, 50.0);
            }
            _ => panic!("Expected Downloading status"),
        }
    }

    #[test]
    fn test_download_status_from_stats_completed() {
        let handle = MockTorrentHandle::new(1, 100, 100, None);
        let stats = handle.torrent_stats();
        let movie = test_movie();
        let status = DownloadStatus::from(&stats, &movie);

        assert!(matches!(status, DownloadStatus::Completed(_)));
    }

    #[test]
    fn test_download_status_from_stats_failed() {
        let handle = MockTorrentHandle::new(1, 50, 100, Some("Network error".to_string()));
        let stats = handle.torrent_stats();
        let movie = test_movie();
        let status = DownloadStatus::from(&stats, &movie);

        match status {
            DownloadStatus::Failed(msg) => {
                assert_eq!(msg, "Network error");
            }
            _ => panic!("Expected Failed status"),
        }
    }

    #[test]
    fn test_active_download_is_completed() {
        let handle = ManagedTorrentHandle {
            inner: Arc::new(MockTorrentHandle::new(1, 100, 100, None)),
        };
        let download = ActiveDownload {
            handle,
            status: DownloadStatus::Completed(DownloadedMovie {
                title: "Test".to_string(),
                path: PathBuf::from("/test"),
                codec: VideoCodec::H264,
                format: RawVideoFormat::MP4,
            }),
            movie: test_movie(),
        };

        assert!(download.is_completed());
        assert!(!download.is_downloading());
        assert!(!download.is_failed());
    }

    #[test]
    fn test_active_download_is_downloading() {
        let handle = ManagedTorrentHandle {
            inner: Arc::new(MockTorrentHandle::new(1, 50, 100, None)),
        };
        let download = ActiveDownload {
            handle,
            status: DownloadStatus::Downloading { progress: 50.0 },
            movie: test_movie(),
        };

        assert!(!download.is_completed());
        assert!(download.is_downloading());
        assert!(!download.is_failed());
    }

    #[test]
    fn test_active_download_downloaded_display() {
        let handle = ManagedTorrentHandle {
            inner: Arc::new(MockTorrentHandle::new(1, 75, 100, None)),
        };
        let download = ActiveDownload {
            handle,
            status: DownloadStatus::Downloading { progress: 75.5 },
            movie: test_movie(),
        };

        assert_eq!(download.downloaded(), "75.5/100");
    }

    #[test]
    fn test_download_progress_percentage_calculation() {
        let handle = MockTorrentHandle::new(1, 256, 1024, None);
        let stats = handle.torrent_stats();

        let progress = DownloadProgress::from(stats);
        assert_eq!(progress.percentage, 25.0);
    }

    #[test]
    fn test_download_progress_zero_total_bytes() {
        let handle = MockTorrentHandle::new(1, 0, 0, None);
        let stats = handle.torrent_stats();

        let progress = DownloadProgress::from(stats);
        assert_eq!(progress.percentage, 0.0);
    }

    #[tokio::test]
    async fn test_download_manager_start_download() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session.clone());
        let movie = test_movie();

        let magnet = MagnetURI("magnet:?xt=urn:btih:test".to_string());
        let result = manager.start_download("123", magnet, movie).await;
        assert!(result.is_ok());

        let downloads = mock_session.get_all_downloads().await;
        assert_eq!(downloads.len(), 1);
    }

    #[tokio::test]
    async fn test_download_manager_stop_download() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session.clone());
        let movie = test_movie();

        mock_session
            .add_torrent("magnet:test", &movie)
            .await
            .unwrap();

        let result = manager.stop_download(TorrentSearchType::ByMovieID(1)).await;
        assert!(result.is_ok());

        let downloads = mock_session.get_all_downloads().await;
        assert_eq!(downloads.len(), 0);
    }

    #[tokio::test]
    async fn test_download_manager_handles_multiple_downloads() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session.clone());

        let mut movie1 = test_movie();
        movie1.id = 1;
        let mut movie2 = test_movie();
        movie2.id = 2;

        let magnet1 = MagnetURI("magnet:?xt=urn:btih:test1".to_string());
        let magnet2 = MagnetURI("magnet:?xt=urn:btih:test2".to_string());

        manager
            .start_download("123", magnet1, movie1)
            .await
            .unwrap();
        manager
            .start_download("456", magnet2, movie2)
            .await
            .unwrap();

        let downloads = mock_session.get_all_downloads().await;
        assert_eq!(downloads.len(), 2);
    }

    #[tokio::test]
    async fn test_download_manager_stop_nonexistent_download() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session);

        let result = manager
            .stop_download(TorrentSearchType::ByMovieID(999))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_download_manager_failed_downloads_tracking() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session.clone());

        manager
            .failed_downloads
            .write()
            .await
            .push(("123".to_string(), "Network error".to_string()));

        let failed = manager.failed_downloads.read().await;
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].0, "123");
        assert_eq!(failed[0].1, "Network error");
    }

    #[test]
    fn test_torrent_id_equality() {
        let id1 = TorrentID::new(1, 100, "tt12345".to_string());
        let id2 = TorrentID::new(1, 100, "tt12345".to_string());
        let id3 = TorrentID::new(2, 100, "tt12345".to_string());

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_download_status_variants() {
        let failed_status = DownloadStatus::Failed("Error".to_string());
        assert!(matches!(failed_status, DownloadStatus::Failed(_)));

        let downloading_status = DownloadStatus::Downloading { progress: 50.0 };
        assert!(matches!(
            downloading_status,
            DownloadStatus::Downloading { .. }
        ));

        let initializing_status = DownloadStatus::Initializing;
        assert!(matches!(initializing_status, DownloadStatus::Initializing));
    }

    #[test]
    fn test_active_processing_initialization() {
        let movie = test_movie();
        let processing = ActiveProcessing::new(movie);

        assert_eq!(processing.status.elapsed, "00:00:00");
        assert_eq!(processing.status.speed, "0x");
        assert_eq!(processing.status.progress, "0%");
    }

    #[test]
    fn test_movie_identifier_to_string() {
        let title_id = MovieIdentifier::Title("The Matrix");
        let tmdb_id = MovieIdentifier::TmdbId("603");

        assert!(title_id.to_string().contains("The Matrix"));
        assert!(tmdb_id.to_string().contains("603"));
    }

    #[test]
    fn test_processing_status_default() {
        let status = ProcessingStatus::default();
        assert_eq!(status.elapsed, "00:00:00");
        assert_eq!(status.speed, "0x");
        assert_eq!(status.progress, "0%");
    }

    #[test]
    fn test_download_progress_display() {
        let handle = MockTorrentHandle::new(1, 512, 1024, None);
        let stats = handle.torrent_stats();
        let progress = DownloadProgress::from(stats);

        assert_eq!(progress.percentage, 50.0);
        assert_eq!(progress.downloaded_bytes, 512);
        assert_eq!(progress.total_bytes, 1024);
    }

    #[tokio::test]
    async fn test_download_manager_concurrent_stop_operations() {
        let mock_session = Arc::new(MockTorrentSession::new());
        let manager = DownloadManager::new_with_session(mock_session.clone());

        let mut movie1 = test_movie();
        movie1.id = 1;
        let mut movie2 = test_movie();
        movie2.id = 2;

        mock_session
            .add_torrent("magnet:test1", &movie1)
            .await
            .unwrap();
        mock_session
            .add_torrent("magnet:test2", &movie2)
            .await
            .unwrap();

        let manager_clone = manager.clone();
        let handle1: tokio::task::JoinHandle<Result<(), Error>> = tokio::spawn(async move {
            manager_clone
                .stop_download(TorrentSearchType::ByMovieID(1))
                .await
        });

        let manager_clone2 = manager.clone();
        let handle2: tokio::task::JoinHandle<Result<(), Error>> = tokio::spawn(async move {
            manager_clone2
                .stop_download(TorrentSearchType::ByMovieID(2))
                .await
        });

        let (result1, result2) = tokio::join!(handle1, handle2);
        assert!(result1.unwrap().is_ok());
        assert!(result2.unwrap().is_ok());

        let downloads = mock_session.get_all_downloads().await;
        assert_eq!(downloads.len(), 0);
    }

    #[test]
    fn test_magnet_uri_display() {
        let magnet = MagnetURI("magnet:?xt=urn:btih:test123".to_string());
        assert_eq!(magnet.to_string(), "magnet:?xt=urn:btih:test123");
    }

    #[test]
    fn test_torrent_search_type_variants() {
        let by_id = TorrentSearchType::ByID(123);
        let by_movie_id = TorrentSearchType::ByMovieID(456);
        let by_tmdb_id = TorrentSearchType::ByTmdbId("789".to_string());

        match by_id {
            TorrentSearchType::ByID(id) => assert_eq!(id, 123),
            _ => panic!("Expected ByID variant"),
        }

        match by_movie_id {
            TorrentSearchType::ByMovieID(id) => assert_eq!(id, 456),
            _ => panic!("Expected ByMovieID variant"),
        }

        match by_tmdb_id {
            TorrentSearchType::ByTmdbId(id) => assert_eq!(id, "789"),
            _ => panic!("Expected ByTmdbId variant"),
        }
    }

    // #[tokio::test]
    // async fn test_search_movie_filters_trusted_uploaders() {
    //     let mock_rss = r#"<?xml version="1.0"?>
    //       <rss>
    //           <channel>
    //               <item>
    //                   <title>Movie.720p</title>
    //                   <link>magnet:...</link>
    //                   <description>Uploader: EZTV</description>
    //                   <attr name="seeders" value="100"/>
    //               </item>
    //               <item>
    //                   <title>Movie.Untrusted</title>
    //                   <link>magnet:...</link>
    //                   <description>Uploader: FakeUploader</description>
    //                   <attr name="seeders" value="200"/>
    //               </item>
    //           </channel>
    //       </rss>"#;

    //     let service = SimpleTorrentService {
    //         http_client: Arc::new(MockHttpClient { response: mock_rss.into() }),
    //         // ...
    //     };

    //     let results = service
    //         .search_movie(&MovieIdentifier::Title("Test"))
    //         .await
    //         .unwrap();
    //     assert_eq!(results.len(), 1);
    //     assert_eq!(results[0].uploader.as_ref().unwrap().0, "EZTV");
    // }
    //
    //
    //
    //
}
