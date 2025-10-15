use anyhow::Result;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use std::{net::Ipv4Addr, sync::Once};

use chrono::DateTime;
use sqlx::{Pool, Sqlite};

use crate::{app::AppBuilder, models::users::User, shared::error::Error};

static INIT: Once = Once::new();

pub async fn setup_test_app() -> Result<(String, Pool<Sqlite>)> {
    INIT.call_once(|| {
        let mut instrumentation = crate::shared::logging::Instrumentation::default();
        instrumentation.verbose = 0;
        instrumentation
            .setup()
            .expect("Failed to setup instrumentation");
    });

    let rand_port = rand::random::<u16>() % 10000 + 10000;
    let app = AppBuilder::new()
        .address(Ipv4Addr::new(127, 0, 0, 1))
        .port(rand_port as usize)
        .tls(false)
        .prod(false)
        .build()
        .await
        .expect("Failed to build test app");

    // setup throughaway database connection
    let db_url = "sqlite::memory:".to_string();
    let db_pool = sqlx::SqlitePool::connect(&db_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");

    let test_valid_user = test_user();

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, display_name, is_admin, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(&test_valid_user.username)        // Individual field
    .bind(&test_valid_user.email)           // Individual field
    .bind(&test_valid_user.password_hash)
    .bind(&test_valid_user.display_name)    // Individual field
    .bind(test_valid_user.is_admin)         // Individual field
    .bind(test_valid_user.created_at)       // Individual field
    .bind(test_valid_user.updated_at)       // Individual field
    .fetch_one(&db_pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            Error::UsernameExists
        } else {
            e.into()
        }
    });

    println!("USER ID: {:?}", user.as_ref().map(|u| u.id));
    let address = app.address().expect("Failed to get app address");

    let return_clone = db_pool.clone();

    tokio::spawn(async move {
        app.run(db_pool).await.expect("Failed to run test app");
    });

    let address = format!("http://{}", address);
    Ok((address, return_clone))
}

pub fn test_user() -> User {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password("hunter42".as_bytes(), &salt)
        .map_err(|_| Error::PasswordHashFailed)
        .unwrap()
        .to_string();

    let some_date = DateTime::from_timestamp(1415923200, 0).expect("Failed to create test date");
    let test_valid_user = User {
        id: 1,
        username: "ferris".to_string(),
        email: "ferris@gmail.com".to_string(),
        password_hash,
        display_name: None,
        is_admin: false,
        created_at: some_date,
        updated_at: some_date,
    };

    test_valid_user
}
