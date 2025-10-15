use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use sqlx::{Pool, Sqlite};

use crate::{
    models::users::{User, UserProfile},
    shared::error::Error,
};

pub struct UserRepository;

impl UserRepository {
    pub async fn find_by_username(
        pool: &Pool<Sqlite>,
        username: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await
    }

    pub async fn add_user(
        user: &User,
        password: String,
        pool: &Pool<Sqlite>,
    ) -> Result<User, Error> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Error::PasswordHashFailed)?
            .to_string();

        sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, display_name, is_admin, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(&user.username)
    .bind(&user.email)
    .bind(&password_hash)
    .bind(&user.display_name)
    .bind(user.is_admin)
    .bind(user.created_at)
    .bind(user.updated_at)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            Error::UsernameExists
        } else {
            e.into()
        }
    })
    }
}

pub struct UserProfileRepository;

impl UserProfileRepository {
    pub async fn from_user_id(pool: &Pool<Sqlite>, user_id: i64) -> Result<UserProfile, Error> {
        let profile = sqlx::query_as!(
            UserProfile,
            r#"
            SELECT
                u.id,
                u.username,
                COALESCE(u.display_name, u.username) as display_name,
                (SELECT COUNT(*) FROM watchlist WHERE user_id = u.id) as watchlist_count,
                (SELECT COUNT(*) FROM watched_movies WHERE user_id = u.id) as watched_count,
                (SELECT COUNT(*) FROM reviews WHERE user_id = u.id) as reviews_count
            FROM users u
            WHERE u.id = $1
            "#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        Ok(profile)
    }
}
