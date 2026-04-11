use snafu::Snafu;

use crate::{handlers::errors::render_error, models::movie::MovieState};

pub use snafu::ResultExt;

// #[derive(Debug, thiserror::Error)]
// pub enum AuthError {
//     #[error("Username already exists")]
//     UsernameExists { username: String },
//     #[error("Email already exists")]
//     EmailExists { email: String },
//     #[error("invalid credentials")]
//     InvalidCredentials,
//     #[error("session expired")]
//     SessionExpired,
//     #[error("session not found")]
//     SessionNotFound,
//     #[error("User not found")]
//     UserNotFound,
//     #[error("password hash failed")]
//     PasswordHashFailed,
//     #[error("session flush failed")]
//     SessionFlushFailed,
//     #[error("session update failed")]
//     SessionUpdateFailed,
//     #[error("tower session error")]
//     SessionLayerError(#[from] tower_sessions::session::Error),
// }
//
//
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum StreamingError {
    #[snafu(display("Invalid range header"))]
    InvalidRangeHeader { header: String },

    #[snafu(display("File not streamable"))]
    FileNotStreamable { path: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum ConverterError {
    #[snafu(display("Unsupported codec: {codec}"))]
    UnsupportedCodec { codec: String },

    #[snafu(display("Unsupported format: {format}"))]
    UnsupportedFormat { format: String },

    #[snafu(display("Conversion failed"))]
    ConversionFailed { reason: String },

    #[snafu(display("Invalid input"))]
    ConverterInvalidInput { reason: String },

    #[snafu(display("No streamable formats available"))]
    NoStreamableFormats,

    #[snafu(display("No suitable converter found"))]
    NoSuitableConverter,

    #[snafu(display("File not available for conversion"))]
    NotAvailableForConversion,

    #[snafu(display("Converter IO error during {operation}"))]
    ConverterIOError {
        operation: &'static str,
        #[snafu(source)]
        source: std::io::Error,
    },

    #[snafu(display("Unsupported conversion from {from} to {to}"))]
    UnsupportedConversion { from: String, to: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum TorrentError {
    #[snafu(display("Torrent search failed for query: {query}"))]
    SearchFailed {
        query: String,
        #[snafu(source)]
        source: reqwest::Error,
    },
    LibrqbError {
        #[snafu(source)]
        source: anyhow::Error,
    },

    #[snafu(display("Download failed"))]
    DownloadFailed { torrent_id: String },

    #[snafu(display("No valid video file found in torrent"))]
    NoValidVideoFile { torrent_id: String },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum ValidationError {
    #[snafu(display("Invalid input for field: {field}"))]
    InvalidInput { field: &'static str, reason: String },

    #[snafu(display("Deserialization failed"))]
    DeserializationFailed {
        #[snafu(source)]
        source: serde_json::Error,
    },
    #[snafu(display("missing field"))]
    MissingField { field: &'static str },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum AuthError {
    #[snafu(display("User not found"))]
    UserNotFound {
        username: String,
        #[snafu(source)]
        source: sqlx::Error,
    },

    #[snafu(display("Invalid CSRF token"))]
    InvalidCsrfToken,
    #[snafu(display("CSRF token verification failed"))]
    CsrfTokenVerificationFailed {
        #[snafu(source)]
        source: axum_csrf::CsrfError,
    },
    #[snafu(display("User not found (simple)"))]
    SimpleUserNotFound { username: String },

    #[snafu(display("Session not found"))]
    SessionNotFound,

    #[snafu(display("Session expired"))]
    SessionExpired,

    #[snafu(display("Invalid credentials"))]
    InvalidCredentials { reason: &'static str },

    #[snafu(display("Password hash failed"))]
    PasswordHashFailed,

    #[snafu(display("Session flush failed"))]
    SessionFlushFailed,

    #[snafu(display("Session update failed"))]
    SessionUpdateFailed,

    #[snafu(display("Session layer error"))]
    SessionLayerError {
        #[snafu(source)]
        source: tower_sessions::session::Error,
    },

    #[snafu(display("internal tower session error during"))]
    TowerSessionError {
        operation: &'static str,
        #[snafu(source)]
        source: tower_sessions::session::Error,
    },
    #[snafu(display("Hashing password failed"))]
    HasherError { operation: &'static str },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum UserError {
    #[snafu(display("User credentials already exists"))]
    UserAlreadyExistsError {
        reason_key: String,
        #[snafu(source)]
        source: sqlx::Error,
    },
}

#[derive(Debug)]
pub enum MovieMissingReason {
    NoFile,
    NoEntryDatabase,
    NotProcessed,
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum MovieError {
    #[snafu(display("Movie could not be found"))]
    MovieNotFoundError {
        movie_id: i64,
        reason: MovieMissingReason,
    },

    #[snafu(display("Movie not found"))]
    SimpleMovieNotFound,

    #[snafu(display("Movie not available"))]
    SimpleMovieNotAvailable,

    #[snafu(display("Failed to fetch poster"))]
    FetchPosterFailed,

    #[snafu(display("requested segment doenst exist"))]
    MovieSegment { movie_id: i64 },
    #[snafu(display("Movie already watchlisted by user"))]
    MovieAlreadyWatchlistedError { movie_id: i64, user_id: i64 },
    #[snafu(display("Movie already watchlisted by user"))]
    WatchlistedMovieNotFound { movie_id: i64, user_id: i64 },
    #[snafu(display("Movie not in available state"))]
    MovieNotAvailableError {
        movie_id: i64,
        movie_state: MovieState,
    },
    #[snafu(display("Movie with ID '{}' not found", movie_id))]
    IndexFileNotFoundError {
        movie_id: i64,
        #[snafu(source)]
        source: tokio::io::Error,
        // #[snafu(implicit)]
        // location: Location,
    },
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(transparent)]
    AuthError {
        source: AuthError,
    },
    #[snafu(transparent)]
    MovieError {
        source: MovieError,
    },
    #[snafu(transparent)]
    StreamingError {
        source: StreamingError,
    },

    #[snafu(transparent)]
    ValidationError {
        source: ValidationError,
    },
    #[snafu(transparent)]
    TorrentError {
        source: TorrentError,
    },
    #[snafu(transparent)]
    ConverterError {
        source: ConverterError,
    },
    DatabaseError {
        operation: &'static str,
        #[snafu(source)]
        source: sqlx::Error,
    },
    ClientRequestError {
        client: &'static str,
        operation: &'static str,
        url: Option<String>,
        #[snafu(source)]
        source: reqwest::Error,
    },
    AskamaRenderError {
        #[snafu(source)]
        source: askama::Error,
    },

    #[snafu(display("Failed to render template"))]
    FailedRenderTemplate,

    #[snafu(display("HTTP error"))]
    Http {
        #[snafu(source)]
        source: axum::http::Error,
    },

    #[snafu(display("io error"))]
    Io {
        operation: &'static str,
        #[snafu(source)]
        source: std::io::Error,
    },
    CustomIo {
        operation: String,
    },
    TokioIo {
        operation: &'static str,
        #[snafu(source)]
        source: tokio::io::Error,
    },

    #[snafu(display("AWS S3 {operation} failed: {source}"))]
    AwsSDK {
        operation: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[snafu(display("FFmpeg error during"))]
    FFmpegError {
        operation: &'static str,
    },
    GenericError {
        reason: String,
    },
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        tracing::error!(
            error = ?self,
            "Handling error response"
        );

        let status = match &self {
            Error::AwsSDK { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            // Auth errors
            Error::AuthError { source } => match source {
                AuthError::UserNotFound { .. } => StatusCode::NOT_FOUND,
                AuthError::SimpleUserNotFound { .. } => StatusCode::NOT_FOUND,
                AuthError::SessionNotFound => StatusCode::UNAUTHORIZED,
                AuthError::SessionExpired => StatusCode::UNAUTHORIZED,
                AuthError::InvalidCredentials { .. } => StatusCode::UNAUTHORIZED,
                AuthError::PasswordHashFailed => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::SessionFlushFailed => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::SessionUpdateFailed => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::SessionLayerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::TowerSessionError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::HasherError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AuthError::InvalidCsrfToken => StatusCode::FORBIDDEN,
                AuthError::CsrfTokenVerificationFailed { .. } => StatusCode::FORBIDDEN,
            },

            // Movie errors
            Error::MovieError { source } => match source {
                MovieError::MovieNotFoundError { .. } => StatusCode::NOT_FOUND,
                MovieError::SimpleMovieNotFound => StatusCode::NOT_FOUND,
                MovieError::SimpleMovieNotAvailable => StatusCode::CONFLICT,
                MovieError::FetchPosterFailed => StatusCode::INTERNAL_SERVER_ERROR,
                MovieError::MovieAlreadyWatchlistedError { .. } => StatusCode::CONFLICT,
                MovieError::WatchlistedMovieNotFound { .. } => StatusCode::BAD_REQUEST,
                MovieError::MovieNotAvailableError { .. } => StatusCode::CONFLICT,
                MovieError::IndexFileNotFoundError { .. } => StatusCode::NOT_FOUND,
                MovieError::MovieSegment { .. } => StatusCode::BAD_REQUEST,
            },

            // Streaming errors
            Error::StreamingError { source } => match source {
                StreamingError::InvalidRangeHeader { .. } => StatusCode::BAD_REQUEST,
                StreamingError::FileNotStreamable { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            },

            // Validation errors
            Error::ValidationError { source } => match source {
                ValidationError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
                ValidationError::DeserializationFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
                ValidationError::MissingField { .. } => StatusCode::BAD_REQUEST,
            },

            // Torrent errors
            Error::TorrentError { source } => match source {
                TorrentError::SearchFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                TorrentError::DownloadFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                TorrentError::NoValidVideoFile { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                TorrentError::LibrqbError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            },

            // Converter errors
            Error::ConverterError { source } => match source {
                ConverterError::UnsupportedCodec { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ConverterError::UnsupportedFormat { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ConverterError::ConversionFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                ConverterError::ConverterInvalidInput { .. } => StatusCode::BAD_REQUEST,
                ConverterError::NoStreamableFormats => StatusCode::INTERNAL_SERVER_ERROR,
                ConverterError::NoSuitableConverter => StatusCode::INTERNAL_SERVER_ERROR,
                ConverterError::NotAvailableForConversion => StatusCode::BAD_REQUEST,
                ConverterError::ConverterIOError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                ConverterError::UnsupportedConversion { .. } => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            },

            // Infrastructure errors
            Error::DatabaseError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::ClientRequestError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::AskamaRenderError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::FailedRenderTemplate => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Http { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Io { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::CustomIo { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::TokioIo { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::FFmpegError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::GenericError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let response = render_error(self);
        (status, axum::response::Html(response)).into_response()
    }
}
