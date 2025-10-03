use askama::Template;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Html,
};
use reqwest::StatusCode;

use crate::{
    app::AppState,
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
