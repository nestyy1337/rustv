use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Pool, prelude::FromRow};
use std::{marker::PhantomData, net::Ipv4Addr, sync::Arc};
use tower_http::trace::TraceLayer;
use tracing::info;

use axum::{Router, extract::State, routing::get};
use tokio::net::TcpListener;

use crate::common::logging::{
    trace_layer_make_span_with, trace_layer_on_request, trace_layer_on_response,
};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub display_name: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn root(State(state): State<Arc<AppState>>) -> &'static str {
    info!("Handling root request");
    let x = sqlx::query_as::<_, User>("SELECT * FROM users LIMIT 1")
        .fetch_one(&state.pool)
        .await
        .expect("Failed to fetch user from database");
    info!("Fetched user: {:?}", x);
    "Hello, World!"
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
            prod: prod,
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

        return Ok(App {
            listener,
            tls: self.tls,
            prod: self.prod,
        });
    }
}

pub struct AppState {
    pool: Pool<sqlx::Sqlite>,
}

impl AppState {
    pub fn new(pool: Pool<sqlx::Sqlite>) -> Self {
        Self { pool: pool }
    }
}

pub struct App {
    listener: TcpListener,
    tls: bool,
    prod: bool,
}

impl App {
    pub async fn run(self, pool: sqlx::SqlitePool) -> Result<()> {
        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(trace_layer_make_span_with)
            .on_request(trace_layer_on_request)
            .on_response(trace_layer_on_response);
        let app = Router::new()
            .route("/", get(root))
            .with_state(Arc::new(AppState::new(pool)))
            .layer(trace_layer);

        axum::serve(self.listener, app).await?;
        Ok(())
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
