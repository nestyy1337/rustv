use askama::Template;
use axum::{extract::State, response::Html};
use std::sync::Arc;

use crate::{
    app::AppState,
    shared::{error::Error, middleware::AuthBackendSqlite, middleware::AuthSession},
};

#[derive(Template)]
#[template(path = "admin/console.html")]
pub struct AdminConsoleTemplate {
    pub csrf_token: String,
}

pub async fn admin_console(
    _auth_session: AuthSession<AuthBackendSqlite>,
    State(_state): State<Arc<AppState>>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let auth_token = csrf_token.authenticity_token().unwrap_or_default();
    let template = AdminConsoleTemplate {
        csrf_token: auth_token,
    };

    let body = template.render().map_err(|e| {
        tracing::error!(error = %e, "Failed to render admin console template");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(body))
}
