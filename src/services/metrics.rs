use crate::services::{
    movie_manager::MovieManager,
    torrent::{DownloadManager, TorrentSession, TorrentSessionManager},
};

#[async_trait::async_trait]
pub trait StateReporter {
    async fn report_movie_manager_state(&self) -> String;
    fn report_system_metrics(&self) -> String;
    async fn report_download_manager_state(&self) -> String;
}

pub struct SimpleStateReporter {
    movie_manager: MovieManager,
    download_manager: DownloadManager<TorrentSessionManager>,
}

impl SimpleStateReporter {
    #[must_use]
    pub fn new(
        movie_manager: MovieManager,
        download_manager: DownloadManager<TorrentSessionManager>,
    ) -> Self {
        Self {
            movie_manager,
            download_manager,
        }
    }
}

#[async_trait::async_trait]
impl StateReporter for SimpleStateReporter {
    async fn report_movie_manager_state(&self) -> String {
        let movies_str = self
            .movie_manager
            .get_available_movies()
            .await
            .iter()
            .map(|movie| format!("{} (ID: {})", movie.title, movie.id))
            .collect::<Vec<String>>()
            .join(", \n");
        format!("Movie manager state contains movies: {:?}\n", &movies_str)
    }

    fn report_system_metrics(&self) -> String {
        "System Metrics:\nCPU Usage: 45%\nMemory Usage: 3.2GB\nDisk Usage: 120GB\n".to_string()
    }

    async fn report_download_manager_state(&self) -> String {
        let downloads_str = self
            .download_manager
            .session
            .get_active_downloads()
            .await
            .iter()
            .map(|download| download.1.downloaded().clone())
            .collect::<Vec<String>>()
            .join(", \n");
        format!(
            "Download manager state contains active downloads: {:?}\n",
            &downloads_str
        )
    }
}
