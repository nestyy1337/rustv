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
                        // *res.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                        //we redirect to login instead
                        *res.status_mut() = http::StatusCode::SEE_OTHER;
                        res.headers_mut().insert(
                            http::header::LOCATION,
                            http::HeaderValue::from_static("/login"),
                        );

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
        self.user.as_ref().is_some_and(|u| u.is_admin)
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
        Ok(x)
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
                session.clear().await;
                session.flush().await.map_err(|e| e.to_string())?;
                user = None;
            }
        } else {
            tracing::warn!("No user found in session");
            // we should create a guest user here if needed
            data = UserAuthData::default();
            session.clear().await;
            session.flush().await.map_err(|e| e.to_string())?;
            user = None;
        }

        let inner = Arc::new(Mutex::new(AuthSessionData {
            user,
            session,
            data,
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

        inner
            .session
            .cycle_id()
            .await
            .map_err(|_| Error::SessionNotFound)?;
        inner.data.user = user.id;
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
            .map_err(|_e| Error::SessionUpdateFailed)?;

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
