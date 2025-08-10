use core::fmt;
use std::{collections::HashMap, pin::Pin, str::FromStr, sync::Arc};

use argon2::{self, PasswordHash, PasswordVerifier};

use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request},
    http::{self, Response, StatusCode, request::Parts},
};
use serde::{Deserialize, Serialize};
use sqlx::{
    SqlitePool,
    types::{JsonValue, time::OffsetDateTime},
};
use tokio::task;
use tower::{Layer, Service};
use tower_sessions::{
    Expiry, Session, SessionStore,
    cookie::time::Duration,
    session::{Id, Record},
};
use tower_sessions_sqlx_store::SqliteStore;
use tracing::info;

use crate::{
    shared::error::Error,
    users::users::{Credentials, User},
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
        let backend = self.backend.clone();
        let data_key = "auth_session_data";

        // Because the inner service can panic until ready, we need to ensure we only
        // use the ready service.
        //
        // See: https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services

        let mut service = self.service.clone();
        Box::pin(async move {
            let Some(session) = req.extensions().get::<Session>().cloned() else {
                tracing::error!("session not found in request extensions");
                let mut res = Response::default();
                *res.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(res);
            };

            let auth_session = match AuthSession::from_session(session, backend.clone()).await {
                Ok(auth_session) => auth_session,
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

            if let Some(ref id) = auth_session.id() {
                tracing::info!(
                    user_id = id.to_string(),
                    "Authenticated user found in session"
                );
            } else {
                tracing::warn!("No authenticated user found in session");
            }

            println!("Session data in request: {:?}", &auth_session);
            req.extensions_mut().insert(auth_session);
            println!(
                "Inserted auth session into request extensions: {:?}",
                req.extensions()
            );

            service.call(req).await
        })
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
    pub inner: AuthSessionData,
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
            tracing::info!("User found in session: {:?}", user);
            let auth = user_auth.session_auth_hash();
            let session_verified = data
                .auth_hash
                .as_ref()
                .is_some_and(|auth_hash| auth_hash == auth);

            if !session_verified {
                info!("Session auth hash does not match user auth hash, resetting session data");
                data = UserAuthData::default();
                session.flush().await.map_err(|e| e.to_string())?;
                user = None;
            }
        } else {
            tracing::warn!("No user found in session");
        }

        let inner = AuthSessionData {
            user,
            session,
            data: data,
        };

        Ok(AuthSession { backend, inner })
    }

    pub fn id(&self) -> Option<Id> {
        self.inner.session.id()
    }

    pub fn authenticate(
        &mut self,
        creds: Credentials,
    ) -> impl Future<Output = Result<Option<User>, Error>> + Send {
        self.backend.authenticate(creds)
    }

    pub async fn login(&mut self, user: User) -> Result<(), Error> {
        let auth_hash = user.session_auth_hash().to_vec();
        self.inner.data.user = user.id.into();
        self.inner.data.auth_hash = Some(auth_hash);

        self.update_session().await
    }

    pub async fn update_session(&mut self) -> Result<(), Error> {
        self.inner
            .session
            .insert("data", &self.inner.data)
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
    // async fn is_authenticated(&self) -> bool;
    // async fn user_id(&self) -> Option<i64>;
    // async fn create_session(&mut self, user_id: i64) -> Result<(), String>;
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

    async fn authenticate(&self, creds: Credentials) -> Result<Option<User>, Self::Error> {
        let user: Option<User> = sqlx::query_as("select * from users where username = ? ")
            .bind(creds.username)
            .fetch_optional(&self.db)
            .await?;

        // Verifying the password is blocking and potentially slow, so we'll do so via
        // `spawn_blocking`.
        task::spawn_blocking(|| {
            // We're using password-based authentication--this works by comparing our form
            // input with an argon2 password hash.
            Ok(user.filter(|user| verify_password(creds.password, &user.password_hash).is_ok()))
        })
        .await?
    }

    async fn get_user(&self, user_id: i64) -> Result<Option<User>, Self::Error> {
        info!("Fetching user with ID: {}", user_id);
        let user = sqlx::query_as("select * from users where id = ?")
            .bind(user_id.to_string())
            .fetch_optional(&self.db)
            .await?;
        info!("Fetched user: {:?}", user);

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
