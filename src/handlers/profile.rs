use askama::Template;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Html,
};

use crate::{
    app::AppState,
    repositories::users::{UserProfileRepository, UserRepository},
    shared::{
        error::{AuthError, Error, SimpleUserNotFoundSnafu},
        middleware::{AuthBackendSqlite, AuthSession},
    },
    views::profile::ProfilePageData,
};

pub async fn get_profile_page(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    tracing::info!(username = %username, "Fetching profile for user");

    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s profile",
            authenticated_username,
            username
        );
        return Err(AuthError::SessionNotFound.into());
    }

    let auth_token = csrf_token.authenticity_token().unwrap();

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(
            SimpleUserNotFoundSnafu {
                username: &username,
            }
            .build(),
        )?;

    let profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;

    let data = ProfilePageData {
        profile,
        csrf_token: auth_token,
    };
    let rendered = data.render().map_err(|e| {
        tracing::error!(error = %e, "Template rendering error");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(rendered))
}

pub async fn get_profile_page_session(
    State(state): State<Arc<AppState>>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    let auth_token = csrf_token.authenticity_token().unwrap();

    let user = UserRepository::find_by_username(&state.pool, &authenticated_username)
        .await?
        .ok_or(
            SimpleUserNotFoundSnafu {
                username: &authenticated_username,
            }
            .build(),
        )?;

    let profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;

    let data = ProfilePageData {
        profile,
        csrf_token: auth_token,
    };
    let rendered = data.render().map_err(|e| {
        tracing::error!(error = %e, "Template rendering error");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(rendered))
}
