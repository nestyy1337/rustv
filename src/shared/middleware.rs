use core::fmt;
use std::{pin::Pin, sync::Arc};

use argon2::{self, PasswordHash, PasswordVerifier};

use axum::{
    extract::{FromRequestParts, Request},
    http::{self, request::Parts, Response, StatusCode},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::{sync::Mutex, task};
use tower::{Layer, Service};
use tower_sessions::{session::Id, Session, SessionStore};
use tower_sessions_sqlx_store::SqliteStore;
use tracing::{debug, info, Instrument};

use crate::{
    models::users::{Credentials, User},
    shared::error::Error,
};

#[derive(Clone, Debug)]
pub struct AuthLayer {
    pub db: SqlitePool,
}

#[derive(Clone, Debug)]
pub struct AuthLayerService<S> {
    service: S,
    backend: AuthBackendSqlite,
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthLayerService<S>;

    fn layer(&self, service: S) -> Self::Service {
        AuthLayerService {
            service,
            backend: AuthBackendSqlite {
                db: self.db.clone(),
            },
        }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for AuthLayerService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Default + Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let span = tracing::info_span!("auth_layer", user = tracing::field::Empty);
        let backend = self.backend.clone();

        let mut service = self.service.clone();
        Box::pin(
            async move {
                let Some(session) = req.extensions().get::<Session>().cloned() else {
                    tracing::error!("session not found in guest extensions");
                    let mut res = Response::default();
                    *res.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(res);
                };

                let auth_session = match AuthSession::from_session(session, backend.clone()).await {
                    Ok(auth_session) => {
                        debug!("Created auth session from session: {:?}", auth_session);
                        auth_session
                    }
                    Err(err) => {
                        tracing::error!(
                            err = %err,
                            "could not create auth session from session"
                        );
                        let mut res = Response::default();
                        *res.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                        return Ok(res);
                    }
                };

                if let Some(ref id) = auth_session.id().await {
                    info!(
                        user_id = id.to_string(),
                        "Authenticated user found in session"
                    );
                } else {
                    tracing::warn!("No authenticated user found in session");
                }

                req.extensions_mut().insert(auth_session);

                service.call(req).await
            }
            .instrument(span),
        )
    }
}

#[derive(Clone, Debug)]
pub struct AuthSessionData {
    user: Option<User>,
    session: Session,
    data: UserAuthData,
}

impl AsRef<Session> for AuthSessionData {
    fn as_ref(&self) -> &Session {
        &self.session
    }
}

impl AuthSessionData {
    pub fn is_user(&self) -> bool {
        self.user.is_some()
    }

    pub fn user_id(&self) -> Option<i64> {
        self.user.as_ref().map(|u| u.id)
    }

    pub fn id(&self) -> Option<Id> {
        self.session.id()
    }

    pub fn is_admin(&self) -> bool {
        self.user.as_ref().map_or(false, |u| u.is_admin)
    }

    pub fn username(&self) -> Option<String> {
        self.user.as_ref().map(|u| u.username.clone())
    }
    pub async fn logout(&mut self) -> Result<(), String> {
        self.session.clear().await;
        self.session.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn data(&self) -> &UserAuthData {
        &self.data
    }
}

impl<S, Backend> FromRequestParts<S> for AuthSession<Backend>
where
    S: Send + Sync + 'static,
    Backend: AuthBackend<SqliteStore> + Send + Sync + 'static + Clone,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let x = parts
            .extensions
            .get::<AuthSession<_>>()
            .cloned()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(x);
    }
}

#[derive(Clone)]
pub struct AuthBackendSqlite {
    db: SqlitePool,
}

#[derive(Clone)]
pub struct AuthSession<Backend: AuthBackend<SqliteStore>> {
    pub backend: Backend,
    pub inner: Arc<Mutex<AuthSessionData>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UserAuthData {
    pub user: i64,
    pub auth_hash: Option<Vec<u8>>,
}

impl AuthSession<AuthBackendSqlite> {
    pub async fn from_session(
        session: Session,
        backend: AuthBackendSqlite,
    ) -> Result<Self, String> {
        let mut data: UserAuthData = session
            .get("data")
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_default();

        let mut user = backend.get_user(data.user).await.unwrap_or_default();

        if let Some(ref user_auth) = user {
            debug!("User found in session: {:?}", user_auth.id);
            let auth = user_auth.session_auth_hash();
            let session_verified = data
                .auth_hash
                .as_ref()
                .is_some_and(|auth_hash| auth_hash == auth);

            if !session_verified {
                debug!("Session auth hash does not match user auth hash, resetting session data");
                data = UserAuthData::default();
                session.flush().await.map_err(|e| e.to_string())?;
                user = None;
            }
        } else {
            tracing::warn!("No user found in session");
        }

        let inner = Arc::new(Mutex::new(AuthSessionData {
            user,
            session,
            data: data,
        }));

        Ok(AuthSession { backend, inner })
    }

    pub async fn id(&self) -> Option<Id> {
        let inner = self.inner.lock().await;
        inner.session.id()
    }

    pub fn authenticate(
        &mut self,
        creds: Credentials,
    ) -> impl Future<Output = Result<Option<User>, Error>> + Send {
        self.backend.authenticate(creds)
    }

    #[tracing::instrument(level = "info", skip_all, fields(user = user.id().to_string()))]
    pub async fn login(&mut self, user: User) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        let auth_hash = user.session_auth_hash().to_vec();

        if inner.data.auth_hash.is_none() {
            debug!("Session auth hash is None, setting it for the first time");
            inner
                .session
                .cycle_id()
                .await
                .map_err(|_| Error::SessionNotFound)?;
        }
        inner.data.user = user.id.into();
        inner.data.auth_hash = Some(auth_hash);
        drop(inner);

        self.update_session().await
    }

    #[tracing::instrument(level = "info", skip_all, fields(user = ?self.inner.lock().await.user))]
    pub async fn logout(&mut self) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        inner.data = UserAuthData::default();
        inner.user = None;

        inner.session.clear().await;

        inner
            .session
            .flush()
            .await
            .map_err(|_| Error::SessionFlushFailed)?;

        Ok(())
    }

    pub async fn is_user(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.is_user()
    }

    pub async fn update_session(&mut self) -> Result<(), Error> {
        let inner = self.inner.lock().await;
        inner
            .session
            .insert("data", &inner.data)
            .await
            .map_err(|e| Error::SessionUpdateFailed)?;

        Ok(())
    }
}

impl fmt::Debug for AuthSession<AuthBackendSqlite> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSession")
            .field("backend", &self.backend)
            .finish()
    }
}

impl fmt::Debug for AuthBackendSqlite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSession")
            .field("backend", &self.db)
            .finish()
    }
}

pub trait AuthBackend<T: SessionStore> {
    type Error;
    fn authenticate(
        &self,
        creds: Credentials,
    ) -> impl Future<Output = Result<Option<User>, Self::Error>> + Send;

    fn get_user(
        &self,
        user_id: i64,
    ) -> impl Future<Output = Result<Option<User>, Self::Error>> + Send;
}

impl AuthBackend<SqliteStore> for AuthBackendSqlite {
    type Error = Error;

    #[tracing::instrument(level = "info", skip_all, fields(user = creds.username))]
    async fn authenticate(&self, creds: Credentials) -> Result<Option<User>, Self::Error> {
        let user: Option<User> = sqlx::query_as("select * from users where username = ? ")
            .bind(creds.username)
            .fetch_optional(&self.db)
            .await?;

        task::spawn_blocking(|| {
            Ok(user.filter(|user| verify_password(creds.password, &user.password_hash).is_ok()))
        })
        .await?
    }

    async fn get_user(&self, user_id: i64) -> Result<Option<User>, Self::Error> {
        debug!("Fetching user with ID: {}", user_id);
        let user = sqlx::query_as("select * from users where id = ?")
            .bind(user_id.to_string())
            .fetch_optional(&self.db)
            .await?;

        Ok(user)
    }
}

pub fn verify_password(password: impl AsRef<[u8]>, hash: &str) -> Result<(), Error> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| Error::PasswordHashFailed)?;
    let verifier: &[&dyn PasswordVerifier] = &[&argon2::Argon2::default()];

    parsed_hash
        .verify_password(verifier, password.as_ref())
        .map_err(|_| Error::PasswordVerificationFailed)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc, time::Duration};

    use chrono::Utc;
    use reqwest::{
        cookie::{CookieStore, Jar},
        Client, StatusCode, Url,
    };

    use crate::shared::test_utils::setup_test_app;

    #[tokio::test]
    async fn test1() {
        let (address, _) = setup_test_app().await.expect("Failed to set up test app");

        let cookie_jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(cookie_jar.clone())
            .build()
            .unwrap();

        let res = client.get(url("/", &address)).send().await.unwrap();
        assert_eq!(
            *res.url(),
            url("/login?next=%2F", &address),
            "Expected redirect to /login after accessing root without authentication"
        );
        assert_eq!(res.status(), StatusCode::OK);

        // Log in with invalid credentials.
        let res = login(&client, "ferris", "bogus", &address).await;
        assert_eq!(
            *res.url(),
            url("/login", &address),
            "Expected redirect to /login after failed login"
        );
        assert_eq!(res.status(), StatusCode::OK);
        // assert!(
        //     cookie_jar.cookies(&url("/"), &a).is_some(),
        //     "Expected cookies (i.e. for flash messages)"
        // );

        let res = login(&client, "ferris", "hunter42", &address).await;
        assert_eq!(
            *res.url(),
            url("/", &address),
            "Expected redirect to / after successful login"
        );
        assert_eq!(res.status(), StatusCode::OK);

        let cookies = cookie_jar
            .cookies(&url("/", &address))
            .expect("A cookie should be set");
        assert!(
            cookies.to_str().unwrap_or("").contains("id="),
            "Expected 'id' cookie to be set after successful login"
        );

        let res = client.get(url("/logout", &address)).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            cookie_jar.cookies(&url("/", &address)).iter().len(),
            0,
            "Expected 'id' cookie to be removed"
        );
    }

    #[tokio::test]
    async fn logout_is_idempotent() {
        let (address, _) = setup_test_app().await.expect("Failed to set up test app");

        let cookie_jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(cookie_jar.clone())
            .build()
            .unwrap();
        login(&client, "ferris", "hunter42", &address).await;

        // Logout twice
        let res1 = client.get(url("/logout", &address)).send().await.unwrap();
        let res2 = client.get(url("/logout", &address)).send().await.unwrap();

        // Both should succeed without errors
        assert_eq!(res1.status(), StatusCode::OK);
        assert_eq!(res2.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn expires_inactive_sessions() {
        let (address, db_pool) = setup_test_app().await.expect("Failed to set up test app");

        let cookie_jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(cookie_jar.clone())
            .build()
            .unwrap();

        let _ = login(&client, "ferris", "hunter42", &address).await;

        let id = cookie_jar
            .cookies(&url("/", &address))
            .expect("A cookie should be set")
            .to_str()
            .expect("Cookie should be valid")
            .split_terminator("=")
            .last()
            .expect("Expected 'id' cookie to be set")
            .to_string();

        sqlx::query("UPDATE tower_sessions SET expiry_date = ? WHERE id = ?")
            .bind(Utc::now().timestamp() - 1)
            .bind(&id)
            .execute(&db_pool)
            .await
            .unwrap();

        let res = client
            .get(url("/protected", &address))
            .send()
            .await
            .unwrap();

        assert_eq!(*res.url(), url("/login?next=%2Fprotected", &address));
    }

    fn url(path: &str, base_address: &str) -> Url {
        let formatted_url = if path.starts_with('/') {
            format!("{base_address}{path}")
        } else {
            format!("{base_address}/{path}")
        };
        formatted_url.parse().unwrap()
    }

    async fn login(
        client: &Client,
        username: &str,
        password: &str,
        base_address: &str,
    ) -> reqwest::Response {
        let mut form = HashMap::new();
        form.insert("username", username);
        form.insert("password", password);
        client
            .post(url("/login", base_address))
            .form(&form)
            .send()
            .await
            .unwrap()
    }
}
