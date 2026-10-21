//! Every query the auth feature runs.
//!
//! Functions take `&mut PgConnection` rather than `&PgPool`: pass `&mut *tx` inside a transaction or
//! `&mut conn` from `pool.acquire()` outside one. Return `sqlx::Result` and let handlers decide which
//! errors become client errors (see `ResultExt::on_constraint`).

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// The public view of a user. Never includes `password_hash`.
#[derive(Debug, serde::Serialize)]
pub struct User {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub bio: String,
    pub image: Option<String>,
}

pub struct Credentials {
    pub user_id: Uuid,
    pub password_hash: String,
}

pub struct Session {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Fails with the `user_username_key` or `user_email_key` constraint on a duplicate.
pub async fn insert_user(
    conn: &mut PgConnection,
    username: &str,
    email: &str,
    password_hash: &str,
) -> sqlx::Result<User> {
    sqlx::query_as!(
        User,
        r#"
            insert into "user" (username, email, password_hash)
            values ($1, $2, $3)
            returning user_id, username, email, bio, image
        "#,
        username,
        email,
        password_hash
    )
    .fetch_one(conn)
    .await
}

pub async fn find_user(conn: &mut PgConnection, user_id: Uuid) -> sqlx::Result<Option<User>> {
    sqlx::query_as!(
        User,
        r#"select user_id, username, email, bio, image from "user" where user_id = $1"#,
        user_id
    )
    .fetch_optional(conn)
    .await
}

/// Email lookup is case-insensitive thanks to the column's collation.
pub async fn find_credentials(
    conn: &mut PgConnection,
    email: &str,
) -> sqlx::Result<Option<Credentials>> {
    sqlx::query_as!(
        Credentials,
        r#"select user_id, password_hash from "user" where email = $1"#,
        email
    )
    .fetch_optional(conn)
    .await
}

pub async fn insert_session(
    conn: &mut PgConnection,
    user_id: Uuid,
    token_hash: &[u8],
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "insert into session (user_id, token_hash, expires_at) values ($1, $2, $3)",
        user_id,
        token_hash,
        expires_at
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Only returns sessions that haven't expired.
pub async fn find_session(
    conn: &mut PgConnection,
    token_hash: &[u8],
) -> sqlx::Result<Option<Session>> {
    sqlx::query_as!(
        Session,
        r#"
            select session_id, user_id, expires_at
            from session
            where token_hash = $1 and expires_at > now()
        "#,
        token_hash
    )
    .fetch_optional(conn)
    .await
}

pub async fn extend_session(
    conn: &mut PgConnection,
    session_id: Uuid,
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "update session set expires_at = $2 where session_id = $1",
        session_id,
        expires_at
    )
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn delete_session(conn: &mut PgConnection, session_id: Uuid) -> sqlx::Result<()> {
    sqlx::query!("delete from session where session_id = $1", session_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn delete_expired_sessions(conn: &mut PgConnection, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query!(
        "delete from session where user_id = $1 and expires_at <= now()",
        user_id
    )
    .execute(conn)
    .await?;
    Ok(())
}
