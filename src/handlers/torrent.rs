use std::sync::Arc;

use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

use crate::{
    app::AppState,
    models::movie::Movie,
    repositories::movies::MovieRepository,
    services::torrent::{
        ActiveDownload, MagnetURI, MagnetURIUnchecked, MovieIdentifier, TorrentID,
        TorrentSearchType, TorrentSession,
    },
    shared::{
        error::{ClientRequestSnafu, DownloadFailedSnafu, Error},
        middleware::VerifiedCSRFToken,
    },
};

#[derive(Template)]
#[template(path = "list.html")]
pub struct TorrentListTemplate {
    pub downloads: Vec<(TorrentID, ActiveDownload)>,
    pub processing: Vec<(String, crate::services::torrent::ActiveProcessing)>,
    pub csrf_token: String,
}

pub async fn list_torrents(
    State(state): State<Arc<AppState>>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, (StatusCode, String)> {
    let downloads = state.downloads.session.get_active_downloads().await;
    let processing = {
        let processing_lock = state.downloads.processing.read().await;
        processing_lock
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };

    let auth_token = csrf_token.authenticity_token().unwrap();
    let template = TorrentListTemplate {
        downloads,
        processing,
        csrf_token: auth_token,
    };

    match template.render() {
        Ok(html) => Ok(Html(html)),
        Err(e) => {
            tracing::error!(error = %e, "Template error");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

#[derive(Template)]
#[template(path = "search.html")]
pub struct TorrentSearchTemplate {
    pub movie: Movie,
    pub torrents: Vec<crate::services::torrent::Item<crate::services::torrent::New>>,
    pub csrf_token: String,
}

pub async fn search_torrents(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<String>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, (StatusCode, String)> {
    let movie = sqlx::query_as!(Movie, "SELECT * FROM movies WHERE id = ?", movie_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Database error");
            (StatusCode::NOT_FOUND, "Movie not found".to_string())
        })?;

    let identifier = MovieIdentifier::Title(&movie.imdb_id);
    let torrents = state
        .torrent_service
        .search_movie(&identifier)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Search error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to search torrents".to_string(),
            )
        })?;

    let auth_token = csrf_token.authenticity_token().unwrap();
    let template = TorrentSearchTemplate {
        movie,
        torrents,
        csrf_token: auth_token,
    };

    match template.render() {
        Ok(html) => Ok(Html(html)),
        Err(e) => {
            tracing::error!(error = %e, "Template error");
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct DownloadRequest {
    magnet_link: String,
    tmdb_id: String,
}

#[derive(Serialize)]
pub struct GenericResponse {
    success: bool,
    message: String,
}

pub async fn download_torrent(
    State(state): State<Arc<AppState>>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<DownloadRequest>,
) -> Result<Json<GenericResponse>, (StatusCode, String)> {
    let movie = MovieRepository::get_movie_by_imdb_id(&state.pool, &payload.tmdb_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Database error fetching movie by TMDB ID");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            )
        })?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Movie not found".to_string()))?;

    let magnet = convert_magnet(&payload.magnet_link, &payload.tmdb_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to convert magnet link");
            (
                StatusCode::BAD_REQUEST,
                "Invalid magnet link provided".to_string(),
            )
        })?;

    let download = state
        .downloads
        .start_download(&payload.tmdb_id, magnet, movie)
        .await;

    Ok(Json(GenericResponse {
        success: download.is_ok(),
        message: match download {
            Ok(_) => "Download started successfully".to_string(),
            Err(e) => {
                tracing::error!(error = %e, "Download error");
                format!("Failed to start download: {}", e)
            }
        },
    }))
}

async fn convert_magnet(jackett_url: &str, tmdb_id: &str) -> Result<MagnetURI, Error> {
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .context(ClientRequestSnafu {
            client: "torrent",
            operation: "building reqwest client for Jackett magnet link retrieval",
            url: jackett_url.to_string(),
        })?;

    tracing::info!(jackett_url = %jackett_url, "Initiating download from Jackett");
    let response = client
        .get(jackett_url)
        .send()
        .await
        .context(ClientRequestSnafu {
            client: "torrent",
            operation: "sending request to Jackett for magnet link",
            url: jackett_url.to_string(),
        })?;

    let unchecked_magnet_url = MagnetURIUnchecked(
        response
            .headers()
            .get("location")
            .ok_or_else(|| {
                DownloadFailedSnafu {
                    torrent_id: tmdb_id.to_string(),
                }
                .build()
            })?
            .to_str()
            .map_err(|_| {
                // shit error
                DownloadFailedSnafu {
                    torrent_id: tmdb_id.to_string(),
                }
                .build()
            })?
            .to_string(),
    );
    MagnetURI::try_from(unchecked_magnet_url)
}

#[derive(Deserialize, Debug)]
pub struct StopDownloadRequest {
    pub tmdb_id: String,
}

pub async fn stop_downloading_torrnet(
    State(state): State<Arc<AppState>>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<StopDownloadRequest>,
) -> Result<Json<GenericResponse>, (StatusCode, String)> {
    let download_progress = state
        .downloads
        .session
        .get_download_by_search(&TorrentSearchType::ByTmdbId(payload.tmdb_id.clone()))
        .await;
    if download_progress.is_none() {
        return Ok(Json(GenericResponse {
            success: false,
            message: "No active download found for the given TMDB ID".to_string(),
        }));
    }
    state
        .downloads
        .stop_download(TorrentSearchType::ByTmdbId(payload.tmdb_id))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to stop download");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to stop download".to_string(),
            )
        })?;

    Ok(Json(GenericResponse {
        success: true,
        message: "Download stopped successfully".to_string(),
    }))
}

#[debug_handler]
pub async fn torrents_status(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<(TorrentID, ActiveDownload)>> {
    let downloads = state.downloads.session.get_active_downloads().await;
    Json(downloads)
}
