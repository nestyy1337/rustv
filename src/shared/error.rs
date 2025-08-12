use tokio::task;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

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
}
