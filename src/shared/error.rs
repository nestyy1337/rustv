use reqwest::StatusCode;
use tokio::task;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    TaskJoin(#[from] task::JoinError),
    #[error("Wrong password")]
    WrongPassword,
    #[error("Failde to build App")]
    AppBuildFailed,
    #[error("Failed to render template")]
    TemplateRender(#[from] askama::Error),
    #[error("Username already exists")]
    UsernameExists,
    #[error("Password hash failed")]
    PasswordHashFailed,
    #[error("Password verification failed")]
    PasswordVerificationFailed,
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Session expired")]
    SessionExpired,
    #[error("Session update failed")]
    SessionUpdateFailed,
    #[error("Session flush failed")]
    SessionFlushFailed,
    #[error("Session clear failed")]
    SessionClearFailed,
    #[error("User not found")]
    UserNotFound,
    #[error("HTTP {0}")]
    Status(StatusCode),
    #[error("Failed to render template")]
    FailedRenderTemplate,
    #[error("database error")]
    DatabaseError(#[from] sqlx::Error),
    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),
    #[error("movie not found")]
    MovieNotFound,
    #[error("tokio io error")]
    TokioIoError(#[from] std::io::Error),
    #[error("invalid movie range")]
    InvalidRange,
    #[error("missing movie range")]
    MissingRange,
    #[error("generic error: {0}")]
    Generic(String),
    #[error("serde json error")]
    SerdeJsonError(#[from] serde_json::Error),
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match &self {
            Error::DatabaseError(e) => {
                if let sqlx::Error::RowNotFound = e {
                    (StatusCode::NOT_FOUND, "Resource not found".to_string())
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Database error".to_string(),
                    )
                }
            }
            Error::WrongPassword => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::AppBuildFailed => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::TemplateRender(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::UsernameExists => (StatusCode::CONFLICT, self.to_string()),
            Error::PasswordHashFailed => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::PasswordVerificationFailed => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::InvalidPassword => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::SessionNotFound => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::SessionExpired => (StatusCode::UNAUTHORIZED, self.to_string()),
            Error::SessionUpdateFailed => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::SessionFlushFailed => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::SessionClearFailed => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Generic(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            Error::TaskJoin(e) => {
                tracing::error!("Task join error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            _ => {
                if let Error::Status(code) = &self {
                    (*code, self.to_string())
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    )
                }
            }
        };

        let body = serde_json::json!({
            "error": error_message,
        });

        let mut response = axum::response::Response::new(axum::body::Body::from(body.to_string()));
        *response.status_mut() = status;
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        response
    }
}
