use anyhow::Result;
use std::{marker::PhantomData, net::Ipv4Addr};

use axum::{Router, routing::get};
use tokio::net::TcpListener;

// basic handler that responds with a static string
async fn root() -> &'static str {
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

pub struct App {
    listener: TcpListener,
    tls: bool,
    prod: bool,
}

impl App {
    pub async fn run(self) -> Result<()> {
        let app = Router::new().route("/", get(root));

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
