//! Users and login sessions.
//!
//! This module is also the reference layout for a feature. Copy this shape for new features:
//!
//! - `mod.rs`: the feature's `router()` plus anything other features may use (here, the extractors).
//! - `handlers.rs`: HTTP only. Extract, validate, call `db`, build the response.
//! - `db.rs`: plain query functions taking `&mut PgConnection`, so the same function works on a pooled
//!   connection or inside a transaction.
//! - Helper modules (`password.rs`, `session.rs`) for logic that is neither HTTP nor SQL.
//!
//! Other features only depend on [`AuthUser`] / [`MaybeAuthUser`].

use axum::{
    Router,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use sqlx::PgPool;
use uuid::Uuid;

use crate::http::{AppState, Error, Result};

mod db;
mod handlers;
mod password;
mod session;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", post(handlers::register))
        .route("/sessions", post(handlers::login).delete(handlers::logout))
        .route("/user", get(handlers::current_user))
}

/// Extractor for handlers that require a logged-in user. Rejects with `401` otherwise.
///
/// ```ignore
/// async fn handler(auth: AuthUser, State(db): State<PgPool>) -> Result<..> { .. auth.user_id .. }
/// ```
///
/// This only checks the session; it doesn't load the user row. Fetch the user yourself if you need more
/// than the ID.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

/// Extractor for handlers where logging in is optional. Never rejects for a missing or invalid session.
#[derive(Debug, Clone, Copy)]
pub struct MaybeAuthUser(pub Option<AuthUser>);

impl<S> FromRequestParts<S> for AuthUser
where
    PgPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        MaybeAuthUser::from_request_parts(parts, state)
            .await?
            .0
            .ok_or(Error::Unauthorized("authentication required"))
    }
}

impl<S> FromRequestParts<S> for MaybeAuthUser
where
    PgPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(cookie) = jar.get(session::COOKIE_NAME) else {
            return Ok(Self(None));
        };

        let db = PgPool::from_ref(state);
        let mut conn = db.acquire().await?;

        let Some(found) = db::find_session(&mut conn, &session::hash_token(cookie.value())).await?
        else {
            return Ok(Self(None));
        };

        if session::needs_refresh(found.expires_at) {
            db::extend_session(&mut conn, found.session_id, session::new_expiry()).await?;
        }

        Ok(Self(Some(AuthUser {
            user_id: found.user_id,
            session_id: found.session_id,
        })))
    }
}
