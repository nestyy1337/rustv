use askama::Template;
use serde::Deserialize;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Html,
    Json,
};
use reqwest::StatusCode;

use crate::{
    app::AppState,
    repositories::watchlist::WatchlistRepository,
    services::watchlist::WatchlistService,
    shared::{
        error::Error,
        middleware::{AuthBackendSqlite, AuthSession},
    },
    views::watchlist::WatchlistView,
};

pub async fn get_watchlist_page(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<Html<String>, Error> {
    tracing::info!("Fetching watchlist for user: {}", username);

    let (profile, watchlist) = WatchlistService::get_user_watchlist(&state.pool, &username).await?;

    let view = WatchlistView::new(profile, watchlist);
    let rendered = view
        .render()
        .map_err(|_| Error::Status(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct AddWatchlistPayload {
    movie_id: i64,
}

pub async fn add_watchlist_movie(
    State(state): State<Arc<AppState>>,
    session: AuthSession<AuthBackendSqlite>,
    Json(payload): Json<AddWatchlistPayload>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);
    let movie_id = payload.movie_id;
    tracing::info!(movie_id, user_id, "Adding a new watchlisted movie for user");
    let result =
        WatchlistService::add_watchlsited_movie(user_id, movie_id, state.pool.clone()).await;
    match result {
        Err(e) => match e {
            Error::Generic(msg) => {
                tracing::warn!(
                    movie_id,
                    user_id,
                    "Failed to add movie to watchlist due to generic error: {}",
                    msg
                );
                return Err(Error::Generic(msg));
            }
            _ => {
                tracing::error!(
                    movie_id,
                    user_id,
                    "Failed to add movie to watchlist: {:?}",
                    e
                );
                return Err(Error::Status(StatusCode::INTERNAL_SERVER_ERROR));
            }
        },

        Ok(_) => {}
    }

    tracing::info!(
        movie_id,
        user_id,
        "Successfully added a new watchlisted movie for user"
    );
    Ok(())
}

pub async fn delete_watchlist_item(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
    session: AuthSession<AuthBackendSqlite>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);

    WatchlistService::remove_from_watchlist(&state.pool, user_id, movie_id).await
}
