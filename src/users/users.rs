use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use argon2::{Argon2, PasswordVerifier, password_hash::PasswordHash};

use crate::shared::error::Error;

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
        let parsed_hash = PasswordHash::new(&self.password_hash)
            .map_err(|_| Error::PasswordVerificationFailed)?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::InvalidPassword)
    }

    pub fn set_password(&mut self, password: String) {
        self.password_hash = password;
    }

    pub fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }

    pub fn id(&self) -> i64 {
        self.id
    }
}

// impl AuthUser for User {
//     type Id = i64;
//
//     fn id(&self) -> Self::Id {
//         self.id
//     }
//
//     fn session_auth_hash(&self) -> &[u8] {
//         self.password_hash.as_bytes()
//     }
// }

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}
