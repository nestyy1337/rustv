use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};

use askama::Template;
use axum::{
    extract::Query,
    http::{Response, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{app::AppState, models::users::User, shared::error::Error};

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NextUrl {
    next: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistrationForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirmation: String,
    pub next: Option<String>,
}

impl From<RegistrationForm> for User {
    fn from(form: RegistrationForm) -> Self {
        User {
            id: 0,
            username: form.username,
            email: form.email,
            password_hash: String::new(),
            display_name: None,
            is_admin: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(self::post::login))
        .route("/login", get(self::get::login))
        .route("/register", post(self::post::register))
        .route("/register", get(self::get::register))
        .route("/logout", get(self::get::logout))
}

mod post {
    use crate::{
        models::users::Credentials,
        shared::middleware::{AuthBackendSqlite, AuthSession},
    };
    use std::sync::Arc;

    use axum::extract::State;

    use tracing::{info, warn};

    use crate::shared::error::Error;

    use super::*;

    pub async fn login(
        mut auth_session: AuthSession<AuthBackendSqlite>,
        Form(creds): Form<Credentials>,
    ) -> impl IntoResponse {
        if auth_session.is_user().await {
            info!("User already authenticated, redirecting");
            return if let Some(ref next) = creds.next {
                Redirect::to(next)
            } else {
                Redirect::to("/")
            }
            .into_response();
        } else {
            info!("User not authenticated, proceeding with login");
        }

        let user = match auth_session.authenticate(creds.clone()).await {
            Ok(Some(user)) => {
                info!("User authenticated: {}", user.username);
                user
            }
            Ok(None) => {
                info!("Authentication failed for user: {}", creds.username);
                let mut login_url = "/login".to_string();
                if let Some(_next) = creds.next {
                    login_url = login_url.to_string();
                };

                return Redirect::to(&login_url).into_response();
            }
            Err(err) => {
                warn!("Authentication failed: {}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        if auth_session.login(user).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        if let Some(ref next) = creds.next {
            Redirect::to(next)
        } else {
            Redirect::to("/")
        }
        .into_response()
    }

    pub async fn register(
        mut auth_session: AuthSession<AuthBackendSqlite>,
        State(state): State<Arc<AppState>>,
        Form(form): Form<RegistrationForm>,
    ) -> impl IntoResponse {
        if form.password != form.password_confirmation {
            return redirect_to_register_with_next(form.next);
        }

        // if form.password.len() < 8 {
        //     return redirect_to_register_with_next(form.next);
        // }
        let password = form.password.clone();
        let next = form.next.clone();

        let user: User = form.into();

        match add_user_test_to_db(&user, password, state.pool.clone()).await {
            Ok(user) => user,
            Err(Error::UsernameExists) => {
                return redirect_to_register_with_next(next);
            }
            Err(_) => {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Auto-login after successful registration
        if auth_session.login(user).await.is_err() {
            return redirect_to_register_with_next(next);
        }

        if let Some(ref next) = next {
            Redirect::to(next)
        } else {
            Redirect::to("/")
        }
        .into_response()
    }
}

mod get {

    use crate::shared::middleware::AuthBackendSqlite;
    use crate::shared::middleware::AuthSession;

    use super::*;

    pub async fn login(Query(NextUrl { next }): Query<NextUrl>) -> Html<String> {
        Html(LoginTemplate { next }.render().unwrap())
    }

    pub async fn register(Query(NextUrl { next }): Query<NextUrl>) -> Html<String> {
        Html(RegisterTemplate { next }.render().unwrap())
    }

    pub async fn logout(mut auth_session: AuthSession<AuthBackendSqlite>) -> impl IntoResponse {
        match auth_session.logout().await {
            Ok(_) => Redirect::to("/login").into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

fn redirect_to_register_with_next(next: Option<String>) -> Response<axum::body::Body> {
    let mut register_url = "/register".to_string();
    if let Some(next) = next {
        register_url = format!("{register_url}?next={next}");
    }
    Redirect::to(&register_url).into_response()
}

async fn add_user_test_to_db(
    user: &User,
    password: String,
    pool: SqlitePool,
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
    .bind(&user.username)        // Individual field
    .bind(&user.email)           // Individual field
    .bind(&password_hash)        // Computed hash
    .bind(&user.display_name)    // Individual field
    .bind(user.is_admin)         // Individual field
    .bind(user.created_at)       // Individual field
    .bind(user.updated_at)       // Individual field
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            Error::UsernameExists
        } else {
            e.into()
        }
    })
}
