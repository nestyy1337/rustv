use askama::Template;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Response},
    response::{Html, IntoResponse},
    Json,
};
use reqwest::StatusCode;

use crate::{
    app::AppState,
    clients::tmdb::TmdbClient,
    models::{
        imdb_stuff::{TmdbMovie, TmdbSearchResult},
        movie::{Movie, WatchedMovie, Watchlist},
    },
    repositories::{
        movies::MovieRepository,
        users::{UserProfileRepository, UserRepository},
        watchlist::WatchlistRepository,
    },
    services::{
        movies::MovieService,
        streaming::{parse_range_header, StreamingService},
        watchlist::WatchlistService,
    },
    shared::{
        config::SETTINGS,
        error::Error,
        middleware::{AuthBackendSqlite, AuthSession},
    },
    views::movie::{MovieDetailsData, RatingsPageData, StealMoviesData, WatchedMovieData},
};

pub async fn get_movie_details(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<Html<String>, Error> {
    let session_guard = auth_session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool)
        .await?
        .ok_or(Error::MovieNotFound)?;

    let watchlisted = MovieService::is_watchlisted(user_id, movie_id, &state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check watchlist status: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    let watched = MovieService::is_watched(user_id, movie_id, &state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check watched status: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    tracing::info!(
        "User {} watchlist status for movie {}: {}",
        user_id,
        movie_id,
        watchlisted
    );

    let user_profile = UserProfileRepository::from_user_id(&state.pool.clone(), user_id).await?;
    let movie_details_data = MovieDetailsData::new(movie, watchlisted, watched, user_profile);
    let body = movie_details_data.render().map_err(|e| {
        tracing::error!("Template rendering error: {}", e);
        Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    Ok(Html(body))
}

pub async fn get_movie_details_json(
    State(state): State<Arc<AppState>>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Movie>, Error> {
    tracing::info!("Fetching details for movie ID: {}", movie_id);
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool)
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

pub async fn search_movies(
    State(state): State<Arc<AppState>>,
    Path(input): Path<String>,
) -> Result<Json<Vec<Movie>>, Error> {
    tracing::info!("Fetching watched movies for search input: {}", input);

    let searched_movies = MovieRepository::search_movie_by_title(&state.pool.clone(), &input)
        .await
        .map_err(|e| {
            tracing::error!("Database query error: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .into();

    Ok(searched_movies)
}

pub async fn search_movies_empty(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Movie>>, Error> {
    let searched_movies = MovieRepository::get_top10_latest_movies(&state.pool.clone())
        .await
        .map_err(|e| {
            tracing::error!("Database query error: {}", e);
            Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?
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
    Json(payload): Json<DeleteWatchlistPayload>,
) -> Result<(), Error> {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);
    let movie_id = payload.movie_id;

    tracing::info!(
        "User {} requested deletion of movie ID {} from watchlist",
        user_id,
        movie_id
    );
    WatchlistService::delete_watchlisted_movie(movie_id, user_id, state.pool.clone()).await
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

pub async fn delete_watched_movie(
    State(state): State<Arc<AppState>>,
    Path((username, movie_id)): Path<(String, i64)>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    if session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?
        != username
    {
        tracing::warn!(
            "User {} attempted to delete {}'s watched movie",
            session_guard
                .username()
                .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }
    drop(session_guard);
    tracing::info!(
        "User {} requested deletion of movie ID {} from watched movies",
        authenticated_user_id,
        movie_id
    );

    Ok(
        MovieRepository::delete_watched_movie(authenticated_user_id, movie_id, &state.pool.clone())
            .await?,
    )
}

pub async fn add_watched_movie(
    State(state): State<Arc<AppState>>,
    Path((username, movie_id)): Path<(String, i64)>,
    auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = auth_session.inner.lock().await;
    let authenticated_user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    if session_guard
        .username()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?
        != username
    {
        tracing::warn!(
            "User {} attempted to add to {}'s watched movies",
            session_guard
                .username()
                .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?,
            username
        );
        return Err(Error::Status(StatusCode::FORBIDDEN));
    }
    drop(session_guard);

    Ok(
        MovieService::add_watched_movie(authenticated_user_id, movie_id, None, &state.pool.clone())
            .await?,
    )
}

pub async fn stream_video(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(movie_id): Path<i64>,
    // session: AuthSession<AuthBackendSqlite>,
) -> Result<impl IntoResponse, Error> {
    // let session_guard = session.inner.lock().await;
    // let _authenticated_user_id = session_guard
    //     .user_id()
    //     .ok_or(Error::Status(axum::http::StatusCode::UNAUTHORIZED))?;
    // drop(session_guard);

    tracing::info!("Received request to stream movie ID: {}", movie_id);
    let file_size = StreamingService::file_size(&state.pool, movie_id).await;
    let file_size = match file_size {
        Ok(size) => size,
        Err(e) => match e {
            Error::MovieNotFound => {
                tracing::error!("Movie ID {} not found", movie_id);
                return Err(Error::MovieNotFound);
            }
            _ => {
                tracing::error!(
                    "Error retrieving file size for movie ID {}: {}",
                    movie_id,
                    e
                );
                return Err(Error::Status(StatusCode::INTERNAL_SERVER_ERROR));
            }
        },
    };
    tracing::info!("File size for movie ID {}: {}", movie_id, file_size);

    let raw_range = headers
        .get("Range")
        .ok_or(Error::MissingRange)?
        .to_str()
        .map_err(|e| {
            tracing::error!("Invalid Range header: {}", e);
            Error::InvalidRange
        })?;
    tracing::info!("Raw Range header: {}", raw_range);
    let range_header = parse_range_header(raw_range, file_size).await;
    println!("Range header after parsing: {:?}", range_header);
    let range_header = match range_header {
        Ok(range) => range,
        Err(e) => {
            tracing::error!("Error parsing Range header: {}", e);
            return Err(e);
        }
    };

    tracing::info!(
        "Parsed Range header for movie ID {}: {:?}",
        movie_id,
        range_header
    );

    let stream =
        StreamingService::stream_video(movie_id, range_header.unwrap(), &state.pool).await?;
    tracing::info!(
        "Streaming movie ID {}: bytes {}-{} of {}",
        movie_id,
        stream.start,
        stream.end,
        stream.file_size
    );
    let body = Body::from_stream(stream.stream);

    Response::builder()
        .status(206)
        .header("Content-Type", "video/mp4")
        .header("Content-Length", stream.content_length.to_string())
        .header("Accept-Ranges", "bytes")
        .body(body)
        .map_err(|e| {
            tracing::error!("Failed to build response: {}", e);
            Error::TokioIoError(std::io::Error::new(std::io::ErrorKind::Other, e))
        })
}

pub async fn test_player(Path(movie_id): Path<i64>) -> Html<String> {
    Html(format!(
        r#"
<!DOCTYPE html>
<html>
<body>
    <video controls width="800">
        <source src="/movies/stream/{}" type="video/mp4">
    </video>
</body>
</html>
    "#,
        movie_id,
    ))
}

pub async fn get_poster(
    Path(movie_id): Path<i64>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, Error> {
    let movie = MovieRepository::get_movie_by_id(movie_id, &state.pool)
        .await?
        .ok_or(Error::MovieNotFound)?;
    let file_path = format!("movies/{}_poster.jpg", movie_id);
    if let Some(file) = tokio::fs::read(&file_path).await.ok() {
        tracing::info!("Serving poster from file: {}", file_path);
        return Ok(Response::builder()
            .status(200)
            .header("Content-Type", "image/jpeg")
            .header("Content-Length", file.len().to_string())
            .body(Body::from(file))
            .map_err(|e| {
                tracing::error!("Failed to build image response: {}", e);
                Error::TokioIoError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?);
    } else {
        // we need to query the tmdb
        let client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
        let movie = client
            .get_movie_details(&movie.imdb_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch movie details from TMDB: {}", e);
                Error::Generic("Failed to fetch movie details".into())
            })?;
        let bytes_png = client.get_poster(movie, movie_id).await.map_err(|e| {
            tracing::error!("Failed to fetch poster from TMDB: {}", e);
            Error::Generic("Failed to fetch poster".into())
        })?;
        return Ok(Response::builder()
            .status(200)
            .header("Content-Type", "image/jpeg")
            .header("Content-Length", bytes_png.len().to_string())
            .body(Body::from(bytes_png))
            .map_err(|e| {
                tracing::error!("Failed to build image response: {}", e);
                Error::TokioIoError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?);
    }
}

pub async fn steal_movies(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
) -> Result<impl IntoResponse, Error> {
    let steal = StealMoviesData {}.render().map_err(|e| {
        tracing::error!("Template rendering error: {}", e);
        Error::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    Ok(Html(steal))
}

pub async fn search_tmdb_by_title(
    Path(title): Path<String>,
) -> Result<Json<Vec<TmdbSearchResult>>, Error> {
    tracing::info!("Searching TMDb for title: {}", title);
    let movies = MovieService::search_tmdb_by_title(&title).await?;
    Ok(Json(movies))
}

#[derive(Deserialize)]
pub struct RequestMoviePayload {
    pub tmdb_id: String,
}

pub async fn request_movie(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    Json(payload): Json<RequestMoviePayload>,
) -> Result<impl IntoResponse, Error> {
    let session_guard = _auth_session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);

    let client = TmdbClient::new(SETTINGS.application.apikeys.tmdb.clone());
    let tmdb_movie = client.get_movie_details(&payload.tmdb_id).await?;
    tracing::info!("Fetched movie details from TMDb: {:?}", tmdb_movie);

    let id = MovieService::add_movie(&tmdb_movie, &state.pool.clone()).await?;

    WatchlistService::add_watchlsited_movie(user_id, id, state.pool.clone()).await?;

    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": id}),
    ))
}

#[derive(Deserialize)]
pub struct DeleteRequestMoviePayload {
    pub movie_id: i64,
}

pub async fn delete_requested_movie(
    State(state): State<Arc<AppState>>,
    _auth_session: AuthSession<AuthBackendSqlite>,
    Json(payload): Json<DeleteRequestMoviePayload>,
) -> Result<Json<Value>, Error> {
    let session_guard = _auth_session.inner.lock().await;
    let user_id = session_guard
        .user_id()
        .ok_or(Error::Status(StatusCode::UNAUTHORIZED))?;
    drop(session_guard);

    WatchlistService::delete_watchlisted_movie(payload.movie_id, user_id, state.pool.clone())
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

    MovieService::delete_movie(payload.movie_id, &state.pool.clone()).await?;
    Ok(Json(
        serde_json::json!({"status": "success", "movie_id": payload.movie_id}),
    ))
}
