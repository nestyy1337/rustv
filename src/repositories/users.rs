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
