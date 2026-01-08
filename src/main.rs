use std::{net::Ipv4Addr, str::FromStr, sync::Arc};

use backend::{
    app::{AppBuilder, AppState},
    services::{
        movie_manager::MovieManager,
        movies::SimpleMovieService,
        torrent::{DownloadManager, SimpleTorrentService},
    },
    shared::{args::InputArgs, config::SETTINGS},
};
use clap::Parser;

#[tokio::main]
async fn main() {
    let config = &SETTINGS;
    let args = InputArgs::parse();
    args.instrumentation
        .setup()
        .expect("failed to initalize instrumentation");

    let db_pool = sqlx::SqlitePool::connect(&config.database.database_path)
        .await
        .expect("failed to connect to database");

    let socket_address = Ipv4Addr::from_str("0.0.0.0").unwrap();

    let movie_service = SimpleMovieService::new(db_pool.clone());
    let movie_manager = MovieManager::initialize(Arc::new(movie_service.clone()), &db_pool).await;
    let converter = backend::services::converter::FFmpegConverter;
    let download_manager = DownloadManager::new().await;

    tokio::spawn({
        let download_manager = download_manager.clone();
        let movie_manager = movie_manager.clone();
        async move { download_manager.monitor_downloads(movie_manager).await }
    });

    let torrent_service = SimpleTorrentService::new(download_manager.clone(), &db_pool);
    let streaming_service =
        backend::services::streaming::SimpleStreamingService::new(movie_manager.clone());

    let metrics_service = backend::services::metrics::SimpleStateReporter::new(
        movie_manager.clone(),
        download_manager.clone(),
    );

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
        .address(socket_address)
        .port(Some(config.application.port.into()))
        .build()
        .await
        .expect("socket must be broken or port taken")
        .with_state(Arc::new(state));

    tracing::info!(
        address = ?app.local_address().unwrap(),
        "Server is running"
    );

    app.run(db_pool).await.expect("main future resolved");
}
