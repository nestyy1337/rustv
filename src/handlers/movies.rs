use askama::Template;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Html,
    Json,
};
use reqwest::StatusCode;

use crate::{
    app::AppState,
    models::movie::{Movie, WatchedMovie},
    repositories::{
        movies::MovieRepository,
        users::{UserProfileRepository, UserRepository},
    },
    services::{movies::MovieService, watchlist::WatchlistService},
    shared::{
        error::Error,
        middleware::{AuthBackendSqlite, AuthSession},
    },
    views::movie::{RatingsPageData, WatchedMovieData},
};

pub async fn get_movie_details(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Movie>, Error> {
    tracing::info!("Fetching details for movie ID: {}", movie_id);
    let movie = MovieRepository::get_movie_by_id(&state.pool, movie_id)
        .await?
        .ok_or(Error::Status(StatusCode::NOT_FOUND))?;

    tracing::info!("Movie found: {:?}", movie);
    Ok(Json(movie))
}

pub async fn get_watched_movies(
    State(_state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Json<Vec<WatchedMovie>>, Error> {
    tracing::info!("Fetching watched movies for username: {}", username);
    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s watched movies",
            authenticated_username,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }

    Ok(Json::default())
}

pub async fn handle_delete_watchlisted_movie(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
    session: AuthSession<AuthBackendSqlite>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);
    tracing::info!(
        "User {} requested deletion of movie ID {} from watchlist",
        user_id,
        movie_id
    );
    Ok(WatchlistService::delete_watchlisted_movie(movie_id, user_id, state.pool.clone()).await?)
}

pub async fn get_profile_ratings(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    session: AuthSession<AuthBackendSqlite>,
) -> Result<Html<String>, Error> {
    tracing::info!("Fetching ratings for user: {}", username);

    let session_guard = session.inner.lock().await;
    let authenticated_username = session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;

    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s ratings",
            authenticated_username,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }
    drop(session_guard);

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(Error::Status(StatusCode::NOT_FOUND))?;

    let profile = UserProfileRepository::from_user_id(&state.pool, user.id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user profile: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    let data = RatingsPageData::new(profile);
    let body = data.render().map_err(|e| {
        tracing::error!("Template rendering error: {}", e);
        Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    Ok(Html(body))
}

pub async fn get_watched_movies_page(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;

    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s watched movies",
            authenticated_username,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }
    drop(session_guard);

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(Error::Status(StatusCode::NOT_FOUND))?;

    let user_profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;
    let watched_movies =
        MovieRepository::find_watched_movies_by_username(&user, state.pool.clone())
            .await?
            .unwrap_or_default();

    let data = WatchedMovieData::new(watched_movies, user_profile);

    let rendered = data.render().map_err(|e| {
        tracing::error!("Failed to render watched_movies template: {}", e);
        Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(Html(rendered))
}
