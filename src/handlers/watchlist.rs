use askama::Template;
use axum_csrf::CsrfToken;
use serde::Deserialize;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    response::{Html, IntoResponse},
};

use crate::{
    app::AppState,
    shared::{
        error::{AskamaRenderSnafu, AuthError, Error, ResultExt},
        middleware::{AuthBackendSqlite, AuthSession, VerifiedCSRFToken},
    },
    views::watchlist::WatchlistView,
};

pub async fn get_watchlist_page(
    csrf_token: CsrfToken,
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, Error> {
    tracing::info!(username = %username, "Fetching watchlist for user");

    let auth_token = csrf_token.authenticity_token().unwrap();
    let (profile, watchlist) = state.movies_manager.get_user_watchlist(&username).await?;

    let view = WatchlistView {
        profile,
        watchlist,
        csrf_token: auth_token,
    };
    let rendered = view.render().context(AskamaRenderSnafu)?;

    Ok(Html(rendered))
}

pub async fn get_watchlist_page_session(
    csrf_token: CsrfToken,
    State(state): State<Arc<AppState>>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    let auth_token = csrf_token.authenticity_token().unwrap();
    let (profile, watchlist) = state.movies_manager.get_user_watchlist(&username).await?;

    let view = WatchlistView {
        profile,
        watchlist,
        csrf_token: auth_token,
    };
    let rendered = view.render().context(AskamaRenderSnafu)?;

    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct AddWatchlistPayload {
    movie_id: i64,
}

pub async fn add_watchlist_movie(
    State(state): State<Arc<AppState>>,
    session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<AddWatchlistPayload>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);
    let movie_id = payload.movie_id;
    state
        .movies_manager
        .add_watchlisted_movie(user_id, movie_id)
        .await?;

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
    _csrf: VerifiedCSRFToken,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    state
        .movies_manager
        .remove_from_watchlist(user_id, movie_id)
        .await?;
    Ok(())
}
