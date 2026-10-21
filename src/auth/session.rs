//! Session tokens and the cookie that carries them.

use axum_extra::extract::cookie::{Cookie, SameSite};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use super::db;

pub const COOKIE_NAME: &str = "session";

const TTL: TimeDelta = TimeDelta::days(30);

/// Writing on every request would turn every read into a write; once a day is plenty for a 30 day TTL.
const REFRESH_AFTER: TimeDelta = TimeDelta::days(1);

/// Creates a session row and returns the raw token to put in the cookie.
pub async fn create(conn: &mut PgConnection, user_id: Uuid) -> sqlx::Result<String> {
    let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    db::insert_session(conn, user_id, &hash_token(&token), new_expiry()).await?;
    Ok(token)
}

pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn new_expiry() -> DateTime<Utc> {
    Utc::now() + TTL
}

pub fn needs_refresh(expires_at: DateTime<Utc>) -> bool {
    expires_at - Utc::now() < TTL - REFRESH_AFTER
}

pub fn cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        // The database is the source of truth for expiry (and it slides), so the cookie itself just needs
        // to outlive any session. 400 days is the maximum browsers honour.
        .max_age(time::Duration::days(400))
        .build()
}

pub fn removal_cookie() -> Cookie<'static> {
    Cookie::build(COOKIE_NAME).path("/").build()
}
