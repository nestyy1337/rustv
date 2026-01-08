use std::path::PathBuf;

use backend::{
    services::torrent::{
        DownloadStatus, MagnetURI, MagnetURIUnchecked, TorrentSearchType, TorrentSession,
    },
    shared::test_utils::{setup_test_app, test_movie},
};

#[tokio::test]
async fn test_torrnet_added() {
    let (addr, state) = setup_test_app().await.unwrap();
    let path = PathBuf::from(state.movies_manager.downloads_path()).join("Big Buck Bunny");
    if path.exists() {
        tokio::fs::remove_dir_all(&path).await.unwrap();
    }

    state
        .downloads
        .start_download(
            "tt99999999",
            MagnetURI::try_from(MagnetURIUnchecked(
                "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny"
                    .to_string(),
            ))
            .unwrap(),
            test_movie(),
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let active_download = state
        .downloads
        .session
        .get_active_downloads()
        .await
        .into_iter()
        .find(|(id, _)| id.tmdb_id == "tt9999999");

    assert!(
        active_download.is_some(),
        "Torrent download was not started properly"
    );
    let download = active_download.unwrap();
    assert!(
        download.0.tmdb_id == "tt9999999",
        "Torrent download has incorrect TMDB ID"
    );
    assert!(
        download.1.movie == test_movie(),
        "Torrent download has incorrect movie associated"
    );
    assert!(
        matches!(download.1.status, DownloadStatus::Downloading { .. }),
        "Torrent download is not in downloading status"
    );
}

#[tokio::test]
async fn test_double_torrent_add() {
    let (addr, state) = setup_test_app().await.unwrap();
    let path = PathBuf::from(state.movies_manager.downloads_path()).join("Big Buck Bunny");
    if path.exists() {
        tokio::fs::remove_dir_all(&path).await.unwrap();
    }

    let result = state
        .downloads
        .start_download(
            "tt99999999",
            MagnetURI::try_from(MagnetURIUnchecked(
                "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny"
                    .to_string(),
            ))
            .unwrap(),
            test_movie(),
        )
        .await;
    assert!(result.is_ok(), "First torrent add should succeed");

    let result = state
        .downloads
        .start_download(
            "tt99999999",
            MagnetURI::try_from(MagnetURIUnchecked(
                "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny"
                    .to_string(),
            ))
            .unwrap(),
            test_movie(),
        )
        .await;
    assert!(
        result.is_err(),
        "Second torrent add for the same movie should fail"
    );
}

#[tokio::test]
async fn test_stop_torrent_download() {
    let (addr, state) = setup_test_app().await.unwrap();
    let path = PathBuf::from(state.movies_manager.downloads_path()).join("Big Buck Bunny");
    if path.exists() {
        tokio::fs::remove_dir_all(&path).await.unwrap();
    }

    state
        .downloads
        .start_download(
            "tt99999999",
            MagnetURI::try_from(MagnetURIUnchecked(
                "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny"
                    .to_string(),
            ))
            .unwrap(),
            test_movie(),
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let stop_result = state
        .downloads
        .stop_download(TorrentSearchType::ByMovieID(test_movie().id))
        .await;
    assert!(stop_result.is_ok(), "Stopping torrent download failed");
    assert!(
        state
            .downloads
            .session
            .get_download_by_search(&TorrentSearchType::ByMovieID(test_movie().id))
            .await
            .is_none(),
        "Torrent download was not removed after stopping"
    );
}
