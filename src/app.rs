use askama::Template;
use aws_config::Region;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use axum::extract::{FromRef, Request};
use axum::http::StatusCode;
use axum::middleware::{Next, from_fn};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{delete, post};
use axum_csrf::{CsrfConfig, CsrfLayer};
use snafu::ResultExt;
use sqlx::Pool;
use std::{marker::PhantomData, net::Ipv4Addr, sync::Arc};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_sessions::{ExpiredDeletion, Session, cookie};
use tower_sessions::{Expiry, SessionManagerLayer, cookie::time::Duration};
use tower_sessions_sqlx_store::SqliteStore;
use tracing::info;

use axum::{Router, extract::State, routing::get};
use tokio::net::TcpListener;

use crate::auth;
use crate::handlers::admin::admin_console;
use crate::handlers::errors::fallback_404;
use crate::handlers::metrics::get_all_metrics;
use crate::shared::config::SETTINGS;
use crate::shared::error::{DatabaseSnafu, Error};

type Result<T> = std::result::Result<T, Error>;
use crate::handlers::movies::{
    add_movie_by_imdb_id, add_watched_movie, delete_requested_movie, delete_watched_movie,
    delete_watchlisted_movie, get_imdb_id_from_tmdb, get_movie_details, get_movie_details_json,
    get_poster, get_profile_ratings, get_profile_ratings_session, get_watched_movies_page,
    get_watched_movies_page_session, movie_details_page_from_imdb_id, request_movie,
    request_movie_tmdb, requested_movies, requested_movies_page, save_progress, search_movies,
    search_movies_empty, search_tmdb_by_title, serve_m3u8, steal_movies, stream_hls_test,
    test_player, update_movie_rating,
};
use crate::handlers::profile::{get_profile_page, get_profile_page_session};
use crate::handlers::torrent::{
    download_torrent, list_torrents, search_torrents, stop_downloading_torrnet, torrents_status,
};
use crate::handlers::watchlist::{
    add_watchlist_movie, get_watchlist_page, get_watchlist_page_session,
};
use crate::models::users::UserProfile;
use crate::repositories::movies::MovieRepository;
use crate::repositories::users::UserProfileRepository;
use crate::services::converter::FFmpegConverter;
use crate::services::movie_manager::MovieManager;
use crate::services::torrent::{DownloadManager, TorrentService, TorrentSessionManager};
use crate::shared::logging::{
    trace_layer_make_span_with, trace_layer_on_request, trace_layer_on_response,
};
use crate::shared::middleware::{AuthBackendSqlite, AuthLayer, AuthSession};
use crate::views::pages::FrontPageData;

#[cfg(feature = "s3")]
pub async fn aws_client() -> aws_sdk_s3::Client {
    let region_provider =
        RegionProviderChain::first_try(Region::new(SETTINGS.application.aws.region.clone()))
            .or_default_provider();

    let shared_config = aws_config::from_env().region(region_provider).load().await;
    let client = Client::new(&shared_config);
    client
}

async fn root(
    State(_state): State<Arc<AppState>>,
    session: AuthSession<AuthBackendSqlite>,
    csrf_token: axum_csrf::CsrfToken,
) -> impl IntoResponse {
    let session_guard = session.inner.lock().await;
    let user_id = session_guard.user_id();
    drop(session_guard);

    let movies = MovieRepository::get_top10_latest_movies(&_state.pool)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = ?e, "Failed to fetch latest movies");
            vec![]
        });

    let user_data = if let Some(uid) = user_id {
        match UserProfileRepository::from_user_id(&_state.pool, uid).await {
            Ok(profile) => profile,
            Err(e) => {
                info!("Failed to fetch user profile: {:?}", e);
                UserProfile::default()
            }
        }
    } else {
        UserProfile::default()
    };

    let auth_token = csrf_token.authenticity_token().unwrap();
    let data = FrontPageData::new(user_data, movies, auth_token);

    Html(data.render().unwrap())
}

pub struct AddressNotSet;
pub struct AddressSet;
pub struct PortNotSet;
pub struct PortSet;

#[derive(Debug, Clone, Copy)]
pub struct AppBuilder<AddressState, PortState> {
    address: Option<Ipv4Addr>,
    port: Option<usize>,
    tls: bool,
    prod: bool,
    _marker: PhantomData<(AddressState, PortState)>,
}

impl Default for AppBuilder<AddressNotSet, PortNotSet> {
    fn default() -> Self {
        Self::new()
    }
}

impl AppBuilder<AddressNotSet, PortNotSet> {
    pub fn new() -> Self {
        Self {
            address: None,
            port: None,
            tls: false,
            prod: false,
            _marker: PhantomData,
        }
    }
}

impl<PortState> AppBuilder<AddressNotSet, PortState> {
    pub fn address<A>(self, address: A) -> AppBuilder<AddressSet, PortState>
    where
        A: Into<Ipv4Addr>,
    {
        AppBuilder {
            address: Some(address.into()),
            port: self.port,
            tls: self.tls,
            prod: self.prod,
            _marker: PhantomData,
        }
    }
}

impl<AddressState> AppBuilder<AddressState, PortNotSet> {
    pub fn port(self, port: Option<usize>) -> AppBuilder<AddressState, PortSet> {
        AppBuilder {
            address: self.address,
            port,
            tls: self.tls,
            prod: self.prod,
            _marker: PhantomData,
        }
    }
}

impl<AddressState, PortState> AppBuilder<AddressState, PortState> {
    pub fn tls(self, tls: bool) -> Self {
        Self {
            address: self.address,
            port: self.port,
            tls,
            prod: self.prod,
            _marker: PhantomData,
        }
    }

    pub fn prod(self, prod: bool) -> Self {
        Self {
            address: self.address,
            port: self.port,
            tls: self.tls,
            prod,
            _marker: PhantomData,
        }
    }
}

///```compile_fail
/// use backend::AppBuilder;
/// let app = AppBuilder::new().build().await;
///```
///
///```compile_fail
/// use backend::AppBuilder;
/// let app = AppBuilder::new().address("8.8.8.8").build().await;
///```
///
///```compile_fail
/// use backend::AppBuilder;
/// let app = AppBuilder::new().port(80).build().await;
///```
///
///```compile_fail
/// use backend::AppBuilder;
/// let app = AppBuilder::new().tls(true).build().await;
///```
///
///```compile_fail
/// use backend::AppBuilder;
/// let app = AppBuilder::new().prod(true).build().await;
///```
impl AppBuilder<AddressSet, PortSet> {
    pub async fn build(self) -> Result<App> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            self.address.expect("address cannot be none"),
            self.port.unwrap_or_default()
        ))
        .await
        .expect("Failed to bind TCP listener");

        Ok(App {
            listener,
            tls: self.tls,
            prod: self.prod,
            app_state: None,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<sqlx::Sqlite>,
    pub downloads: DownloadManager<TorrentSessionManager>,
    // pub movies_manager: MovieManager<NaiveMovieStorage>,
    pub movies_manager: MovieManager,
    pub converter: FFmpegConverter,
    pub torrent_service: Arc<dyn TorrentService + Send + Sync>,
    pub streaming_service: Arc<dyn crate::services::streaming::StreamingService + Send + Sync>,
    pub metrics_service: Arc<dyn crate::services::metrics::StateReporter + Send + Sync>,
}

impl FromRef<AppState> for Pool<sqlx::Sqlite> {
    fn from_ref(app_state: &AppState) -> Pool<sqlx::Sqlite> {
        app_state.pool.clone()
    }
}
impl FromRef<AppState> for DownloadManager<TorrentSessionManager> {
    fn from_ref(app_state: &AppState) -> DownloadManager<TorrentSessionManager> {
        app_state.downloads.clone()
    }
}
impl FromRef<AppState> for FFmpegConverter {
    fn from_ref(app_state: &AppState) -> FFmpegConverter {
        app_state.converter.clone()
    }
}
impl FromRef<AppState> for Arc<dyn TorrentService + Send + Sync> {
    fn from_ref(app_state: &AppState) -> Arc<dyn TorrentService + Send + Sync> {
        app_state.torrent_service.clone()
    }
}

impl FromRef<AppState> for Arc<dyn crate::services::streaming::StreamingService + Send + Sync> {
    fn from_ref(
        app_state: &AppState,
    ) -> Arc<dyn crate::services::streaming::StreamingService + Send + Sync> {
        app_state.streaming_service.clone()
    }
}
impl FromRef<AppState> for Arc<dyn crate::services::metrics::StateReporter + Send + Sync> {
    fn from_ref(
        app_state: &AppState,
    ) -> Arc<dyn crate::services::metrics::StateReporter + Send + Sync> {
        app_state.metrics_service.clone()
    }
}

impl AppState {
    pub async fn new(
        pool: Pool<sqlx::Sqlite>,
        movie_manager: MovieManager,
        download_manager: DownloadManager<TorrentSessionManager>,
        converter: FFmpegConverter,
        torrent_service: Arc<dyn TorrentService + Send + Sync>,
        streaming_service: Arc<dyn crate::services::streaming::StreamingService + Send + Sync>,
        metrics_service: Arc<dyn crate::services::metrics::StateReporter + Send + Sync>,
    ) -> Self {
        Self {
            pool,
            movies_manager: movie_manager,
            downloads: download_manager,
            converter,
            torrent_service,
            streaming_service,
            metrics_service,
        }
    }
}

pub struct App {
    listener: TcpListener,
    tls: bool,
    prod: bool,
    app_state: Option<Arc<AppState>>,
}

impl App {
    pub fn with_state(self, app_state: Arc<AppState>) -> Self {
        Self {
            listener: self.listener,
            tls: self.tls,
            prod: self.prod,
            app_state: Some(app_state),
        }
    }
}

pub async fn require_auth(
    session: Session,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if let Ok(Some(_user_id)) = session.get::<String>("user_id").await {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub async fn require_auth_redirect(
    session: AuthSession<AuthBackendSqlite>,
    request: Request,
    next: Next,
) -> Response {
    if session.is_user().await {
        info!("User is logged in, proceeding to next handler");
        next.run(request).await
    } else {
        info!("User is not logged in, redirecting to login page");
        let login_url = format!("/login?next={}", urlencoding::encode(request.uri().path()));
        Redirect::to(&login_url).into_response()
    }
}

impl App {
    pub async fn run(self, pool: sqlx::SqlitePool) -> Result<()> {
        let csrf_config = CsrfConfig::default()
            .with_cookie_same_site(cookie::SameSite::Lax)
            .with_secure(false);

        let session_store = SqliteStore::new(pool.clone());
        session_store.migrate().await.context(DatabaseSnafu {
            operation: "migrating session store database",
        })?;

        let _deletion_task = tokio::spawn({
            session_store
                .clone()
                .continuously_delete_expired(tokio::time::Duration::from_secs(60))
        });

        let auth_layer = AuthLayer { db: pool.clone() };

        let static_dir = ServeDir::new("./static");

        let _not_protected = Router::new().route("/nprot", get(root));

        let protected_route = Router::new()
            .route("/protected", get(root))
            .route("/profile/{username}", get(get_profile_page))
            .route("/profile", get(get_profile_page_session))
            .route("/watched/{username}", get(get_watched_movies_page))
            .route("/watched", get(get_watched_movies_page_session))
            .route("/reviews/{username}", get(get_profile_ratings))
            .route("/reviews", get(get_profile_ratings_session))
            .route("/", get(root))
            .route("/watch/{movie_id}", get(test_player))
            .route("/steal", get(steal_movies))
            .route(
                "/movie/imdb/{imdb_id}",
                get(movie_details_page_from_imdb_id),
            )
            .route("/admin", get(admin_console))
            .route("/admin/torrents", get(list_torrents))
            .route("/admin/torrents/search/{movie_id}", get(search_torrents))
            .route("/movies/{movie_id}", get(get_movie_details))
            .route("/requested", get(requested_movies_page))
            .route("/movies/stream/{movie_id}/index.m3u8", get(serve_m3u8))
            .route("/movies/stream/{movie_id}/{segment}", get(stream_hls_test))
            .route("/movies/progress/{movie_id}", post(save_progress))
            .route("/watchlist/{username}", get(get_watchlist_page))
            .route("/watchlist", get(get_watchlist_page_session))
            .route_layer(from_fn(require_auth_redirect));

        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_same_site(cookie::SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(Duration::hours(1)));

        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(trace_layer_make_span_with)
            .on_request(trace_layer_on_request)
            .on_response(trace_layer_on_response);

        let api_router = Router::new()
            .route(
                "/watchlist",
                delete(delete_watchlisted_movie).post(add_watchlist_movie),
            )
            .route("/movie/{movie_id}", get(get_movie_details_json))
            .route("/movie/imdb/{imdb_id}", post(add_movie_by_imdb_id))
            .route("/torrents/status", get(torrents_status))
            .route("/movie/download", post(download_torrent))
            .route("/movie/download/stop", post(stop_downloading_torrnet))
            .route("/movie/search/{input}", get(search_movies))
            .route("/movie/search/", get(search_movies_empty))
            .route("/requested", get(requested_movies))
            .route(
                "/movie/request",
                post(request_movie).delete(delete_requested_movie),
            )
            .route("/movie/request/tmdb_id", post(request_movie_tmdb))
            .route(
                "/watched/{username}/{movie_id}",
                delete(delete_watched_movie)
                    .post(add_watched_movie)
                    .put(update_movie_rating),
            )
            .route("/movie/tmdb/search/{title}", get(search_tmdb_by_title))
            .route("/movie/tmdb/{tmdb_id}/imdb", get(get_imdb_id_from_tmdb))
            .route("/movie/{movie_id}/poster", get(get_poster))
            .route("/metrics", get(get_all_metrics));

        let app = Router::new()
            .merge(protected_route)
            .merge(auth::login::router())
            .fallback(fallback_404())
            .nest("/api", api_router)
            .nest_service("/static", static_dir)
            .with_state(self.app_state.expect("AppState must be set"))
            .layer(CsrfLayer::new(csrf_config))
            .layer(auth_layer)
            .layer(session_layer)
            .layer(trace_layer);

        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("Failed to run the server");
        Ok(())
    }

    pub fn address(&self) -> Option<String> {
        self.listener.local_addr().ok().map(|addr| addr.to_string())
    }

    pub fn is_tls_enabled(&self) -> bool {
        self.tls
    }
    pub fn is_prod(&self) -> bool {
        self.prod
    }
    pub fn local_address(&self) -> Option<std::net::SocketAddr> {
        self.listener.local_addr().ok()
    }
}

pub async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    info!("Received shutdown signal, shutting down gracefully...");
}

#[cfg(unix)]
pub async fn terminate() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM signal");
    sigterm.recv().await;
    info!("Received SIGTERM, shutting down gracefully...");
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    #[tokio::test]
    async fn test_app_builder() {
        let app = AppBuilder::new()
            .address(Ipv4Addr::new(127, 0, 0, 1))
            .port(Some(8080))
            .build()
            .await;

        assert!(
            app.is_ok(),
            "AppBuilder should succeed with valid address and port"
        );

        let app = app.unwrap();

        assert_eq!(
            app.local_address().unwrap().ip(),
            Ipv4Addr::new(127, 0, 0, 1),
            "Local address should match the one set in AppBuilder"
        );

        assert_eq!(
            app.local_address().unwrap().port(),
            8080,
            "Port should match the one set in AppBuilder"
        );

        assert!(!app.is_tls_enabled(), "TLS should be disabled by default");

        assert!(
            !app.is_prod(),
            "Production mode should be disabled by default"
        );
    }
}
