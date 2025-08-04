use std::net::Ipv4Addr;

use backend::{
    app::AppBuilder,
    common::config::{SETTINGS, Settings},
};
use tracing::info;

#[tokio::main]
async fn main() {
    let config = &SETTINGS;
    let port = config.application.port;
    println!("port: {}", port);

    let socketaddress = Ipv4Addr::new(0, 0, 0, 0);
    let app = AppBuilder::new()
        .address(socketaddress)
        .port(8080)
        .build()
        .await
        .expect("socket must be broken or port taken");
    info!("Server is running");
    app.run().await.expect("main future resolved");
}
