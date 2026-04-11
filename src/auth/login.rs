use std::{ops::Deref, sync::Arc};

use askama::Template;
use axum::{
    Form, Router,
    extract::Query,
    http::{Response, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::Deserialize;

use crate::{app::AppState, models::users::User};

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    next: Option<String>,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub struct NextUrl {
    next: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    next: Option<String>,
    pub csrf_token: String,
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
        .route("/logout", post(self::get::logout))
}

///```compile_fail
/// let valid_url = LoginNextURL("/malicious".to_string());
///```
#[derive(Clone, Debug, Deserialize)]
pub struct LoginNextURLUnchecked(pub String);

impl Deref for LoginNextURLUnchecked {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

mod internal {
    use std::ops::{Deref, DerefMut};

    use crate::auth::login::LoginNextURLUnchecked;

    #[derive(Clone)]
    pub struct LoginNextURL(String);

    impl Deref for LoginNextURL {
        type Target = str;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for LoginNextURL {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Default for LoginNextURL {
        fn default() -> Self {
            LoginNextURL("/".to_string())
        }
    }

    impl TryFrom<LoginNextURLUnchecked> for LoginNextURL {
        type Error = ();
        fn try_from(unchecked: LoginNextURLUnchecked) -> Result<Self, ()> {
            if unchecked.starts_with("//") {
                Err(())
            } else {
                Ok(LoginNextURL(unchecked.0))
            }
        }
    }

    impl std::fmt::Display for LoginNextURL {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
}

impl Default for LoginNextURLUnchecked {
    fn default() -> Self {
        LoginNextURLUnchecked("/".to_string())
    }
}

pub use internal::LoginNextURL;

mod post {
    use crate::{
        models::users::Credentials,
        repositories::users::UserRepository,
        shared::middleware::{AuthBackendSqlite, AuthSession},
    };
    use std::sync::Arc;

    use axum::extract::State;

    use tracing::{info, warn};

    use crate::shared::error::Error;

    use super::{
        AppState, Form, IntoResponse, LoginNextURL, Redirect, RegistrationForm, Response,
        StatusCode, User, redirect_to_register_with_next,
    };

    pub async fn login(
        mut auth_session: AuthSession<AuthBackendSqlite>,
        csrf_token: axum_csrf::CsrfToken,
        Form(creds): Form<Credentials>,
    ) -> impl IntoResponse {
        let redirect_url =
            LoginNextURL::try_from(creds.next.clone().unwrap_or_default()).unwrap_or_default();

        if auth_session.is_user().await {
            info!("User already authenticated, redirecting");
            return Response::builder()
                .status(StatusCode::OK)
                .header("HX-REDIRECT", "/")
                .body(axum::body::Body::empty())
                .unwrap();
        }

        let user = match auth_session.authenticate(&creds).await {
            Ok(Some(user)) => {
                info!("User authenticated: {}", user.username);
                user
            }
            Ok(None) => {
                info!("Authentication failed for user: {}", creds.username);
                let mut login_url = "/login".to_string();
                login_url = format!("{login_url}?next={redirect_url}");
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

        let resp = Response::builder()
            .status(StatusCode::OK)
            .header("HX-REDIRECT", &*redirect_url)
            .body(axum::body::Body::empty())
            .unwrap();
        (csrf_token, resp).into_response()
    }

    pub async fn register(
        mut auth_session: AuthSession<AuthBackendSqlite>,
        State(state): State<Arc<AppState>>,
        Form(form): Form<RegistrationForm>,
    ) -> Result<impl IntoResponse, Error> {
        if form.password != form.password_confirmation {
            return Ok(redirect_to_register_with_next(form.next));
        }

        // if form.password.len() < 8 {
        //     return redirect_to_register_with_next(form.next);
        // }
        let password = form.password.clone();
        let next = form.next.clone();
        let user: User = form.into();

        UserRepository::add_user(&user, password, &state.pool).await?;

        if auth_session.login(user).await.is_err() {
            return Ok(redirect_to_register_with_next(next));
        }

        Ok(if let Some(ref next) = next {
            Redirect::to(next)
        } else {
            Redirect::to("/")
        }
        .into_response())
    }
}

mod get {

    use crate::shared::middleware::AuthBackendSqlite;
    use crate::shared::middleware::AuthSession;

    use super::{
        Html, IntoResponse, LoginTemplate, NextUrl, Query, RegisterTemplate, StatusCode, Template,
    };

    // even though we have SameSite=Lax, we still want to protect against CSRF at the login page,
    // since login forms can be a target for CSRF attacks, so that the attacker can log the user
    // into an account they control, in this case we generate a CSRF token for the login page, then
    // reset it upon successful login and attach it to the session cookie of the logged user.
    pub async fn login(
        Query(NextUrl { next }): Query<NextUrl>,
        csrf_token: axum_csrf::CsrfToken,
    ) -> impl IntoResponse {
        let auth_token = csrf_token.authenticity_token().unwrap();
        (
            csrf_token,
            Html(
                LoginTemplate {
                    next,
                    csrf_token: auth_token,
                }
                .render()
                .unwrap(),
            ),
        )
            .into_response()
    }

    pub async fn register(
        Query(NextUrl { next }): Query<NextUrl>,
        csrf_token: axum_csrf::CsrfToken,
    ) -> Html<String> {
        let auth_token = csrf_token.authenticity_token().unwrap();
        Html(
            RegisterTemplate {
                next,
                csrf_token: auth_token,
            }
            .render()
            .unwrap(),
        )
    }

    pub async fn logout(mut auth_session: AuthSession<AuthBackendSqlite>) -> impl IntoResponse {
        match auth_session.logout().await {
            Ok(()) => ([("HX-REDIRECT", "/")], ()).into_response(),
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
