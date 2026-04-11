use askama::Template;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::Response,
    response::{Html, IntoResponse},
};

use crate::{
    app::AppState,
    clients::tmdb::TmdbClient,
    models::{
        imdb_stuff::TmdbSearchResult,
        movie::{Movie, MovieState, WatchedMovie},
    },
    repositories::{
        movies::MovieRepository,
        users::{UserProfileRepository, UserRepository},
        watchlist::WatchlistRepository,
    },
    services::streaming::{IndexLocation, SegmentLocation},
    shared::{
        config::SETTINGS,
        error::{
            AskamaRenderSnafu, AuthError, ClientRequestSnafu, Error, HttpSnafu, MovieError,
            MovieMissingReason, MovieNotFoundSnafu, ResultExt, SessionNotFoundSnafu,
            SimpleMovieNotFoundSnafu, SimpleUserNotFoundSnafu, TokioIoSnafu,
        },
        middleware::{AuthBackendSqlite, AuthSession, VerifiedCSRFToken},
    },
    views::movie::{
        MovieDetailsData, MoviePlayerData, RatingsPageData, RequestedMoviesTemplate,
        StealMoviesData, WatchedMovieData,
    },
};

pub async fn get_movie_details(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionExpired)?;
    drop(session_guard);

    tracing::Span::current().record("user_id", user_id);
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool)
        .await?
        .ok_or(
            MovieNotFoundSnafu {
                movie_id,
                reason: MovieMissingReason::NoEntryDatabase,
            }
            .build(),
        )?;

    let mut watched = false;
    let mut streamable_video = None;
    if movie.state.is_available() {
        watched = state.movies_manager.is_watched(user_id, movie_id).await?;
        streamable_video = state.movies_manager.get_streamable(movie.id).await?;
    }

    let tmdb_movie = MovieRepository::get_tmdb_movie_by_imdb_id(&movie.imdb_id, &state.pool)
        .await?
        .ok_or(MovieError::SimpleMovieNotFound)?;

    let watchlisted = state
        .movies_manager
        .is_watchlisted(user_id, movie_id)
        .await?;

    tracing::info!(
        movie_id = movie_id,
        user_id = user_id,
        watchlisted = watchlisted,
        watched = watched,
        "Fetched movie details"
    );

    let user_profile = UserProfileRepository::from_user_id(&state.pool.clone(), user_id).await?;
    let auth_token = csrf_token.authenticity_token().unwrap();
    let movie_details_data = MovieDetailsData {
        movie,
        tmdb_movie,
        streamable: streamable_video,
        watchlisted,
        watched,
        profile: user_profile,
        csrf_token: auth_token,
    };
    let body = movie_details_data.render().map_err(|e| {
        tracing::error!(error = %e, "Template rendering error");
        Error::FailedRenderTemplate
    })?;
    Ok(Html(body))
}

pub async fn get_movie_details_json(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Movie>, Error> {
    tracing::info!(movie_id = movie_id, "Fetching details for movie");
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool)
        .await?
        .ok_or(
            MovieNotFoundSnafu {
                movie_id,
                reason: MovieMissingReason::NoEntryDatabase,
            }
            .build(),
        )?;

    tracing::info!(movie = ?movie, "Movie found");
    Ok(Json(movie))
}

pub async fn get_watched_movies(
    State(_state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Json<Vec<WatchedMovie>>, Error> {
    tracing::info!(username = %username, "Fetching watched movies for username");
    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s watched movies",
            authenticated_username,
            username
        );
        return Err(SimpleUserNotFoundSnafu { username }.build().into());
    }
    unreachable!();

    // Ok(Json::default())
}

pub async fn search_movies(
    State(state): State<Arc<AppState>>,
    Path(input): Path<String>,
) -> Result<Json<Vec<Movie>>, Error> {
    tracing::info!(input = %input, "Fetching watched movies for search input");

    let searched_movies = MovieRepository::search_movie_by_title(&state.pool.clone(), &input)
        .await?
        .into();

    Ok(searched_movies)
}

pub async fn search_movies_empty(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Movie>>, Error> {
    let searched_movies = MovieRepository::get_top10_latest_movies(&state.pool.clone())
        .await?
        .into();
    Ok(searched_movies)
}

#[derive(Deserialize, Serialize)]
pub struct DeleteWatchlistPayload {
    pub movie_id: i64,
}

pub async fn delete_watchlisted_movie(
    State(state): State<Arc<AppState>>,
    session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<DeleteWatchlistPayload>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);
    let movie_id = payload.movie_id;

    tracing::info!(
        "User {} requested deletion of movie ID {} from watchlist",
        user_id,
        movie_id
    );
    state
        .movies_manager
        .remove_from_watchlist(user_id, movie_id)
        .await
}

pub async fn get_profile_ratings(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    tracing::info!(username = %username, "Fetching ratings for user");

    let session_guard = session.inner.lock().await;
    let authenticated_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s ratings",
            authenticated_username,
            username
        );
        return Err(SimpleUserNotFoundSnafu { username }.build().into());
    }

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or_else(|| SimpleUserNotFoundSnafu { username }.build())?;

    let profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;
    let rated_movies = MovieRepository::find_rated_movies_by_username(&user, state.pool.clone())
        .await?
        .unwrap_or_default();

    let auth_token = csrf_token.authenticity_token().unwrap();
    let data = RatingsPageData::new(rated_movies, profile, auth_token);
    let body = data.render().map_err(|e| {
        tracing::error!(error = %e, "Template rendering error");
        Error::FailedRenderTemplate
    })?;
    Ok(Html(body))
}

pub async fn get_profile_ratings_session(
    State(state): State<Arc<AppState>>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(
            SimpleUserNotFoundSnafu {
                username: &username,
            }
            .build(),
        )?;

    let user_profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;
    let rated_movies = MovieRepository::find_rated_movies_by_username(&user, state.pool.clone())
        .await?
        .unwrap_or_default();

    let auth_token = csrf_token.authenticity_token().unwrap();
    let data = RatingsPageData::new(rated_movies, user_profile, auth_token);

    let rendered = data.render().map_err(|e| {
        tracing::error!(error = %e, "Failed to render ratings template");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(rendered))
}

pub async fn get_watched_movies_page_session(
    State(state): State<Arc<AppState>>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(
            SimpleUserNotFoundSnafu {
                username: &username,
            }
            .build(),
        )?;

    let user_profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;
    let watched_movies =
        MovieRepository::find_watched_movies_by_username(&user, state.pool.clone())
            .await?
            .unwrap_or_default();

    let auth_token = csrf_token.authenticity_token().unwrap();
    let data = WatchedMovieData::new(watched_movies, user_profile, auth_token);

    let rendered = data.render().map_err(|e| {
        tracing::error!(error = %e, "Failed to render watched_movies template");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(rendered))
}

pub async fn get_watched_movies_page(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;

    if authenticated_username != username {
        tracing::warn!(
            "User {} attempted to access {}'s watched movies",
            authenticated_username,
            username
        );
        return Err(SessionNotFoundSnafu {}.build())?;
    }
    drop(session_guard);

    let user = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(
            SimpleUserNotFoundSnafu {
                username: &username,
            }
            .build(),
        )?;

    let user_profile = UserProfileRepository::from_user_id(&state.pool, user.id).await?;
    let watched_movies =
        MovieRepository::find_watched_movies_by_username(&user, state.pool.clone())
            .await?
            .unwrap_or_default();

    let auth_token = csrf_token.authenticity_token().unwrap();
    let data = WatchedMovieData::new(watched_movies, user_profile, auth_token);

    let rendered = data.render().map_err(|e| {
        tracing::error!(error = %e, "Failed to render watched_movies template");
        Error::FailedRenderTemplate
    })?;

    Ok(Html(rendered))
}

pub async fn delete_watched_movie(
    State(state): State<Arc<AppState>>,
    Path((username, movie_id)): Path<(String, i64)>,
    auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    let session_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);
    if username != session_username {
        tracing::warn!(
            "User {} attempted to delete from {}'s watched movies",
            session_username,
            username
        );
        return Err(SimpleUserNotFoundSnafu { username }.build().into());
    }
    tracing::info!(
        "User {} requested deletion of movie ID {} from watched movies",
        authenticated_user_id,
        movie_id
    );

    MovieRepository::delete_watched_movie(authenticated_user_id, movie_id, &state.pool.clone())
        .await
}

pub async fn add_watched_movie(
    State(state): State<Arc<AppState>>,
    Path((username, movie_id)): Path<(String, i64)>,
    auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    let session_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);
    if username != session_username {
        tracing::warn!(
            "User {} attempted to add to {}'s watched movies",
            username,
            session_username
        );
        return Err(SimpleUserNotFoundSnafu { username }.build().into());
    }

    state
        .movies_manager
        .add_watched_movie(authenticated_user_id, movie_id, None)
        .await
}

#[derive(Deserialize)]
pub struct UpdateRatingRequest {
    pub rating: f32,
}

pub async fn update_movie_rating(
    State(state): State<Arc<AppState>>,
    Path((username, movie_id)): Path<(String, i64)>,
    auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<UpdateRatingRequest>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    let session_username = session_guard.username().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    if username != session_username {
        tracing::warn!(
            "User {} attempted to update rating for {}'s movies",
            session_username,
            username
        );
        return Err(SimpleUserNotFoundSnafu { username }.build().into());
    }

    state
        .movies_manager
        .add_watched_movie(authenticated_user_id, movie_id, Some(payload.rating))
        .await
}

pub async fn test_player(
    Path(movie_id): Path<i64>,
    auth_session: AuthSession<AuthBackendSqlite>,
    State(state): State<Arc<AppState>>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<impl IntoResponse, Error> {
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool).await;
    let movie = match movie {
        Ok(Some(m)) => m,
        Ok(None) => {
            tracing::warn!(movie_id = movie_id, "Movie not found");
            return Err(MovieError::SimpleMovieNotFound.into());
        }
        Err(e) => {
            tracing::error!(error = %e, "Database error while fetching movie");
            return Err(e);
        }
    };
    if !movie.state.is_available() {
        return Err(MovieError::SimpleMovieNotAvailable.into());
    }

    let mut session_guard = auth_session.inner.lock().await;
    let last_known_timestamp: usize = session_guard
        .session_mut()
        .get_value(format!("movie_{}", movie_id).as_ref())
        .await
        .unwrap_or(Some(serde_json::Value::Number(0.into())))
        .and_then(|v| v.as_str().map(|s| s.parse::<usize>().unwrap_or(0)))
        .unwrap_or(0);

    drop(session_guard);

    let auth_token = csrf_token.authenticity_token().unwrap();

    Ok(Html(
        MoviePlayerData {
            timestamp: last_known_timestamp,
            movie: &movie,
            csrf_token: auth_token,
        }
        .render()
        .context(AskamaRenderSnafu)?,
    ))
}

pub async fn save_progress(
    Path(movie_id): Path<i64>,
    auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, Error> {
    let mut session_guard = auth_session.inner.lock().await;
    let _user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;

    let timestamp = payload
        .get("timestamp")
        .and_then(|v| v.as_u64())
        .ok_or(MovieError::FetchPosterFailed)?
        .to_string();

    session_guard
        .session_mut()
        .insert(format!("movie_{}", movie_id).as_ref(), &timestamp)
        .await
        .map_err(|e| AuthError::SessionLayerError { source: e })?;

    tracing::info!(timestamp = %timestamp, movie = %movie_id, "saved last known timestamp");
    Ok(Json(serde_json::json!({"status": "success"})))
}

// FIXME: inner functionality needs to be wrapped into a MovieManager method,
// that internally is calling MovieStorage since we want to be able to serve posters
// from CDNs or file system.
pub async fn get_poster(
    Path(movie_id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let poster = state.movies_manager.get_poster(movie_id).await?;
    if let Some(file) = poster {
        Ok(Response::builder()
            .status(200)
            .header("Content-Type", "image/jpeg")
            .header("Content-Length", file.len().to_string())
            .body(Body::from(file))
            .context(HttpSnafu)?)
    } else {
        tracing::warn!(movie_id = movie_id, "Poster not found for movie");
        Err(MovieError::FetchPosterFailed.into())
    }
}

pub async fn steal_movies(
    State(_state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<impl IntoResponse, Error> {
    let auth_token = csrf_token.authenticity_token().unwrap();
    let steal = StealMoviesData {
        csrf_token: auth_token,
    }
    .render()
    .context(AskamaRenderSnafu)?;
    Ok(Html(steal))
}

pub async fn search_tmdb_by_title(
    Path(title): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<TmdbSearchResult>>, Error> {
    tracing::info!(title = %title, "Searching TMDb for title");
    let movies = state
        .movies_manager
        .movie_service
        .search_tmdb_by_title(&title)
        .await?;
    Ok(Json(movies))
}

pub async fn get_imdb_id_from_tmdb(Path(tmdb_id): Path<String>) -> Result<Json<Value>, Error> {
    tracing::info!(tmdb_id = %tmdb_id, "Getting IMDB ID from TMDB ID");
    let client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
    let tmdb_movie = client.get_movie_details(&tmdb_id).await?;

    let imdb_id = tmdb_movie.imdb_id.ok_or(MovieError::SimpleMovieNotFound)?;

    Ok(Json(serde_json::json!({
        "imdb_id": imdb_id
    })))
}

#[derive(Deserialize)]
pub struct RequestMovieIDPayload {
    pub movie_id: i64,
}

pub async fn request_movie(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<RequestMovieIDPayload>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = _auth_session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    MovieRepository::mark_requested(payload.movie_id, &state.pool).await?;
    state
        .movies_manager
        .add_watchlisted_movie(user_id, payload.movie_id)
        .await?;

    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": payload.movie_id}),
    ))
}

#[derive(Deserialize)]
pub struct RequestMovieTmdbPayload {
    pub tmdb_id: String,
}

pub async fn request_movie_tmdb(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<RequestMovieTmdbPayload>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = _auth_session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);
    let tmdb_client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());

    let tmdb_movie = tmdb_client.get_movie_details(&payload.tmdb_id).await?;

    let movie = MovieRepository::get_movie_by_imdb_id(
        &state.pool,
        &tmdb_movie.imdb_id.ok_or(MovieError::SimpleMovieNotFound)?,
    )
    .await?
    .ok_or(MovieError::SimpleMovieNotFound)?;

    MovieRepository::mark_requested(movie.id, &state.pool).await?;
    state
        .movies_manager
        .add_watchlisted_movie(user_id, movie.id)
        .await?;

    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": movie.id}),
    ))
}

#[derive(Deserialize)]
pub struct DeleteRequestMoviePayload {
    pub movie_id: i64,
}

pub async fn delete_requested_movie(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
    Json(payload): Json<DeleteRequestMoviePayload>,
) -> Result<Json<Value>, Error> {
    let session_guard = _auth_session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionNotFound)?;
    drop(session_guard);

    state
        .movies_manager
        .remove_from_watchlist(user_id, payload.movie_id)
        .await?;

    if WatchlistRepository::is_watchlisted_anywhere(&state.pool.clone(), payload.movie_id).await? {
        tracing::info!(
            "Movie ID {} is still watchlisted by other users, not deleting",
            payload.movie_id
        );
        return Ok(Json(
            serde_json::json!({"status": "success", "movie_id": payload.movie_id}),
        ));
    }

    state.movies_manager.remove_movie(payload.movie_id).await?;

    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": payload.movie_id}),
    ))
}

pub async fn requested_movies(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Json<Vec<Movie>>, Error> {
    let movies = MovieRepository::find_requested_movies(&state.pool.clone()).await?;
    Ok(Json(movies))
}

pub async fn requested_movies_page(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let movies = MovieRepository::find_requested_movies(&state.pool).await?;

    let auth_token = csrf_token.authenticity_token().unwrap();
    let template = RequestedMoviesTemplate {
        movies,
        csrf_token: auth_token,
    };

    let rendered = template.render().context(AskamaRenderSnafu)?;

    Ok(Html(rendered))
}

pub async fn serve_m3u8(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
) -> Result<impl IntoResponse, Error> {
    let m3u8_content = state.movies_manager.get_m3u8_content(movie_id).await;
    let m3u8_content = match m3u8_content {
        Ok(content) => content,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get M3U8 content");
            return Err(e);
        }
    };
    match m3u8_content {
        IndexLocation::Local(path) => {
            let content = tokio::fs::read_to_string(&path)
                .await
                .context(TokioIoSnafu {
                    operation: "reading m3u8 content",
                })?;

            Response::builder()
                .status(200)
                .header("Content-Type", "application/vnd.apple.mpegurl")
                .header("Content-Length", content.len().to_string())
                .body(Body::from(content))
                .context(HttpSnafu)
        }
        IndexLocation::Remote(url) => {
            let content = reqwest::get(url)
                .await
                .context(ClientRequestSnafu {
                    operation: "fetching remote m3u8 content",
                    client: "reqwest",
                    url: None,
                })?
                .text()
                .await
                .context(ClientRequestSnafu {
                    operation: "reading remote m3u8 content",
                    client: "reqwest",
                    url: None,
                })?;
            Response::builder()
                .status(200)
                .header("Content-Type", "application/vnd.apple.mpegurl")
                .header("Content-Length", content.len().to_string())
                .body(Body::from(content))
                .context(HttpSnafu)
        }
    }
}

///```compile_fail
/// use crate::handlers::movies::HlsSegment;
/// let valid_segment = HlsSegment("test.ts".to_string());
/// ```
#[derive(Debug, Deserialize)]
pub struct HlsSegmentUnchecked(String);

impl HlsSegmentUnchecked {
    pub fn validate(self) -> Result<internal::HlsSegment, MovieError> {
        internal::HlsSegment::try_from(self)
    }
}

impl Deref for HlsSegmentUnchecked {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for HlsSegmentUnchecked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DerefMut for HlsSegmentUnchecked {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[allow(dead_code)]
mod internal {
    use crate::{handlers::movies::HlsSegmentUnchecked, shared::error::MovieError};

    #[derive(Debug)]
    pub struct HlsSegment(String);

    impl std::fmt::Display for HlsSegment {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl HlsSegment {
        pub fn new(s: String) -> Option<Self> {
            if s.starts_with("hls") {
                Some(HlsSegment(s))
            } else {
                None
            }
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl AsRef<str> for HlsSegment {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }

    impl TryFrom<HlsSegmentUnchecked> for HlsSegment {
        type Error = MovieError;

        fn try_from(value: HlsSegmentUnchecked) -> Result<Self, Self::Error> {
            let segment = value.0;
            if segment.contains("..") || segment.contains('/') || segment.contains('\\') {
                tracing::warn!(segment = %segment, "Invalid HLS segment path detected");
                return Err(MovieError::SimpleMovieNotFound);
            }
            Ok(HlsSegment(segment))
        }
    }
}

pub async fn stream_hls_test(
    Path((movie_id, segment)): Path<(i64, HlsSegmentUnchecked)>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    tracing::info!(
        movie_id = movie_id,
        segment = %segment,
        "Received request for HLS segment"
    );

    let segment_source = state
        .movies_manager
        .segment_bytes(movie_id, segment)
        .await?;

    match segment_source {
        SegmentLocation::Local(content) => Response::builder()
            .status(200)
            .header("Content-Type", "video/mp2t")
            .header("Content-Length", content.len().to_string())
            .body(Body::from(content))
            .context(HttpSnafu),
        SegmentLocation::Remote(url) => {
            println!("Redirecting to remote segment URL: {}", url);
            Response::builder()
                .status(302)
                .header("Location", url)
                .body(Body::empty())
                .context(HttpSnafu)
        }
    }
}

pub async fn add_movie_by_imdb_id(
    Path(imdb_id): Path<String>,
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    _csrf: VerifiedCSRFToken,
) -> Result<impl IntoResponse, Error> {
    let tmdb_client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
    let tmdb_movie = tmdb_client.get_movie_details(&imdb_id).await?;
    MovieRepository::insert_tmdb_movie(&tmdb_movie, &state.pool).await?;
    let id = MovieRepository::add_movie(&Movie::from(tmdb_movie), &state.pool).await?;
    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": id}),
    ))
}

pub async fn movie_details_page_from_imdb_id(
    Path(imdb_id): Path<String>,
    State(state): State<Arc<AppState>>,
    auth_session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let user_id = session_guard.user_id().ok_or(AuthError::SessionExpired)?;
    drop(session_guard);

    let tmdb_movie = MovieRepository::get_tmdb_movie_by_imdb_id(&imdb_id, &state.pool)
        .await?
        .ok_or(SimpleMovieNotFoundSnafu {}.build())?;

    let movie = MovieRepository::get_movie_by_imdb_id(&state.pool, &imdb_id)
        .await?
        .ok_or(MovieError::SimpleMovieNotFound)?;

    let mut streamable_video = None;
    let mut watched = false;
    if movie.state == MovieState::Available {
        streamable_video = state.movies_manager.get_streamable(movie.id).await?;
        watched = state.movies_manager.is_watched(user_id, movie.id).await?;
    }

    let watchlisted = state
        .movies_manager
        .is_watchlisted(user_id, movie.id)
        .await?;

    tracing::info!(
        user_id = user_id,
        movie_id = imdb_id,
        watchlisted = watchlisted,
        "User watchlist status for movie"
    );

    let user_profile = UserProfileRepository::from_user_id(&state.pool.clone(), user_id).await?;
    let auth_token = csrf_token.authenticity_token().unwrap();
    let movie_details_data = MovieDetailsData::new(
        movie.clone(),
        tmdb_movie,
        streamable_video,
        watchlisted,
        watched,
        user_profile,
        auth_token,
    );
    let body = movie_details_data.render().context(AskamaRenderSnafu)?;
    Ok(Html(body))
}

#[cfg(test)]
mod tests {
    use crate::handlers::movies::{HlsSegmentUnchecked, internal::HlsSegment};

    #[test]
    fn validate_hls_segment_success() {
        let valid_seg = HlsSegmentUnchecked("segment03.ts".to_string());
        assert!(HlsSegment::try_from(valid_seg).is_ok())
    }

    #[test]
    fn validate_hls_segment_failure() {
        let invalid_seg = HlsSegmentUnchecked("../secret.txt".to_string());
        assert!(HlsSegment::try_from(invalid_seg).is_err())
    }

    #[test]
    fn validate_hls_segment_failure_slash() {
        let invalid_seg = HlsSegmentUnchecked("folder/segment.ts".to_string());
        assert!(HlsSegment::try_from(invalid_seg).is_err())
    }

    #[test]
    fn validate_hls_segment_failure_backslash() {
        let invalid_seg = HlsSegmentUnchecked("folder\\segment.ts".to_string());
        assert!(HlsSegment::try_from(invalid_seg).is_err())
    }
}
