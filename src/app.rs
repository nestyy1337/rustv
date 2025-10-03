use anyhow::Result;
use askama::Template;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{from_fn, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::delete;
use sqlx::Pool;
use std::{marker::PhantomData, net::Ipv4Addr, sync::Arc};
use time::OffsetDateTime;
use tower_http::trace::TraceLayer;
use tower_sessions::{cookie, ExpiredDeletion, Session};
use tower_sessions::{cookie::time::Duration, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;
use tracing::info;

use axum::{extract::State, routing::get, Router};
use tokio::net::TcpListener;

use crate::auth;
use crate::handlers::movies::{
    get_movie_details, get_watched_movies_page, handle_delete_watchlisted_movie,
};
use crate::handlers::profile::get_profile_page;
use crate::handlers::watchlist::get_watchlist_page;
use crate::models::movie::Movie;
use crate::models::users::UserProfile;
use crate::repositories::users::UserProfileRepository;
use crate::shared::logging::{
    trace_layer_make_span_with, trace_layer_on_request, trace_layer_on_response,
};
use crate::shared::middleware::{AuthBackendSqlite, AuthLayer, AuthSession};
use crate::views::pages::FrontPageData;

async fn root(
    State(_state): State<Arc<AppState>>,
    session: AuthSession<AuthBackendSqlite>,
) -> impl IntoResponse {
    let user_id = if session.is_user().await {
        session.inner.lock().await.user_id()
    } else {
        None
    };
    let movies = vec![Movie {
        id: 1,
        title: "Example Movie".to_string(),
        director: "Jane Doe".to_string(),
        release_year: 2023,
        genre: "Drama".to_string(),
        created_at: Some(OffsetDateTime::now_utc()),
        updated_at: Some(OffsetDateTime::now_utc()),
    }];

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

    let data = FrontPageData::new(user_data, movies);

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
    pub fn port(self, port: usize) -> AppBuilder<AddressState, PortSet> {
        AppBuilder {
            address: self.address,
            port: Some(port),
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
            self.port.expect("port cannot be none")
        ))
        .await?;

        Ok(App {
            listener,
            tls: self.tls,
            prod: self.prod,
        })
    }
}

pub struct AppState {
    pub pool: Pool<sqlx::Sqlite>,
}

impl AppState {
    pub fn new(pool: Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

pub struct App {
    listener: TcpListener,
    tls: bool,
    prod: bool,
}

pub async fn require_auth(
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
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
        let session_store = SqliteStore::new(pool.clone());
        session_store.migrate().await?;

        let _deletion_task = tokio::spawn({
            session_store
                .clone()
                .continuously_delete_expired(tokio::time::Duration::from_secs(60))
        });

        let auth_layer = AuthLayer { db: pool.clone() };

        let protected_route = Router::new()
            .route("/", get(root))
            .route_layer(from_fn(require_auth_redirect));

        let session_layer = SessionManagerLayer::new(session_store)
            .with_secure(false)
            .with_same_site(cookie::SameSite::Lax)
            .with_expiry(Expiry::OnInactivity(Duration::hours(1)));

        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(trace_layer_make_span_with)
            .on_request(trace_layer_on_request)
            .on_response(trace_layer_on_response);

        let app = Router::new()
            .route("/test", get(root))
            .merge(protected_route)
            .merge(auth::login::router())
            .route("/profile/{username}", get(get_profile_page))
            .route("/movies/{movie_id}", get(get_movie_details))
            .route("/watchlist/{username}", get(get_watchlist_page))
            .route("/watched/{username}", get(get_watched_movies_page))
            .route(
                "/watchlist/movie/{movie_id}",
                delete(handle_delete_watchlisted_movie),
            )
            .with_state(Arc::new(AppState::new(pool)))
            .layer(auth_layer)
            .layer(session_layer)
            .layer(trace_layer);

        axum::serve(self.listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
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
    use tokio::signal::unix::{signal, SignalKind};

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
            .port(8080)
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
