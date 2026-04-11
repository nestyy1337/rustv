use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::{
    auth::login::LoginNextURLUnchecked,
    shared::error::{Error, InvalidCredentialsSnafu},
};

#[derive(Debug, Serialize, FromRow)]
pub struct UserProfile {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub watchlist_count: i64,
    pub watched_count: i64,
    pub reviews_count: i64,
    pub is_admin: bool,
}

impl Default for UserProfile {
    fn default() -> Self {
        UserProfile {
            id: 0,
            username: "guest".to_string(),
            display_name: "Guest".to_string(),
            watchlist_count: 0,
            watched_count: 0,
            reviews_count: 0,
            is_admin: false,
        }
    }
}

impl UserProfile {
    #[must_use]
    pub fn initials(&self) -> String {
        self.display_name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }
}

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

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("password", &"hidden")
            .field("email", &self.email)
            .field("display_name", &self.display_name)
            .field("is_admin", &self.is_admin)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl User {
    pub fn verify_password(&self, password: String) -> Result<(), Error> {
        let parsed_hash = PasswordHash::new(&self.password_hash).map_err(|_| {
            InvalidCredentialsSnafu {
                reason: "invalid password hash format",
            }
            .build()
        })?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| {
                InvalidCredentialsSnafu {
                    reason: "password verification failed",
                }
                .build()
            })?;
        Ok(())
    }

    pub fn set_password(&mut self, password: String) {
        self.password_hash = password;
    }

    #[must_use]
    pub fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }

    #[must_use]
    pub fn id(&self) -> i64 {
        self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub next: Option<LoginNextURLUnchecked>,
}
