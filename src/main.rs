use std::net::Ipv4Addr;

use backend::{
    app::AppBuilder,
    args::InputArgs,
    common::config::{SETTINGS, Settings},
};
use clap::{Args, Parser};
use tracing::{debug, info, trace};

#[tokio::main]
async fn main() {
    let args = InputArgs::parse();
    args.instrumentation
        .setup()
        .expect("failed to initalize instrumentation");

    let config = &SETTINGS;

    let db_pool = sqlx::SqlitePool::connect(&config.database.database_path)
        .await
        .expect("failed to connect to database");

    let socketaddress = Ipv4Addr::new(0, 0, 0, 0);
    let app = AppBuilder::new()
        .address(socketaddress)
        .port(config.application.port.into())
        .build()
        .await
        .expect("socket must be broken or port taken");

    info!("Server is running at: {}", app.local_address().unwrap());
    app.run(db_pool).await.expect("main future resolved");
}
