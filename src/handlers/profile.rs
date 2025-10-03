use askama::Template;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Html,
};
use reqwest::StatusCode;

use crate::{
    app::AppState,
    repositories::users::{UserProfileRepository, UserRepository},
    shared::{
        error::Error,
        middleware::{AuthBackendSqlite, AuthSession},
    },
    views::profile::ProfilePageData,
};

#[axum::debug_handler]
pub async fn get_profile_page(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Html<String>, Error> {
    tracing::info!("Fetching profile for user: {}", username);

    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s profile",
            authenticated_username,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(Error::Status(StatusCode::NOT_FOUND))?;

    let profile = UserProfileRepository::from_user_id(&state.pool, user.id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user profile: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    let data = ProfilePageData { profile };
    let rendered = data.render().map_err(|e| {
        tracing::error!("Template rendering error: {}", e);
        Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(Html(rendered))
}
