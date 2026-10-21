use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use sqlx::PgPool;
use validator::Validate;

use super::{AuthUser, db, password, session};
use crate::{
    config::Config,
    http::{Error, Result, ResultExt},
};

#[derive(Deserialize, Validate)]
pub struct Register {
    #[validate(length(min = 1, max = 32, message = "must be 1 to 32 characters"))]
    username: String,
    #[validate(email(message = "must be a valid email"))]
    email: String,
    #[validate(length(min = 8, max = 256, message = "must be 8 to 256 characters"))]
    password: String,
}

#[derive(Deserialize)]
pub struct Login {
    email: String,
    password: String,
}

/// Same response whether the email or the password was wrong, so login can't be used to probe for accounts.
const INVALID_LOGIN: Error = Error::Unauthorized("invalid email or password");

/// `POST /api/users`: create an account and log straight in.
pub async fn register(
    State(db): State<PgPool>,
    State(config): State<Arc<Config>>,
    jar: CookieJar,
    Json(req): Json<Register>,
) -> Result<impl IntoResponse> {
    req.validate().map_err(Error::validation_errors)?;

    // Hash before opening the transaction so we don't hold a connection during the slow part.
    let password_hash = password::hash(req.password).await?;

    // The user and their first session are created atomically: no account without a way to log in.
    let mut tx = db.begin().await?;

    let user = db::insert_user(&mut tx, &req.username, &req.email, &password_hash)
        .await
        .on_constraint("user_username_key", |_| {
            Error::unprocessable_entity([("username", "username taken")])
        })
        .on_constraint("user_email_key", |_| {
            Error::unprocessable_entity([("email", "email taken")])
        })?;

    let token = session::create(&mut tx, user.user_id).await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        jar.add(session::cookie(token, config.cookie_secure)),
        Json(user),
    ))
}

/// `POST /api/sessions`: log in with email and password.
pub async fn login(
    State(db): State<PgPool>,
    State(config): State<Arc<Config>>,
    jar: CookieJar,
    Json(req): Json<Login>,
) -> Result<impl IntoResponse> {
    let mut conn = db.acquire().await?;

    let credentials = db::find_credentials(&mut conn, &req.email)
        .await?
        .ok_or(INVALID_LOGIN)?;

    if !password::verify(req.password, credentials.password_hash).await? {
        return Err(INVALID_LOGIN);
    }

    db::delete_expired_sessions(&mut conn, credentials.user_id).await?;
    let token = session::create(&mut conn, credentials.user_id).await?;

    let user = db::find_user(&mut conn, credentials.user_id)
        .await?
        .ok_or(Error::NotFound)?;

    Ok((
        jar.add(session::cookie(token, config.cookie_secure)),
        Json(user),
    ))
}

/// `DELETE /api/sessions`: log out of the current session only.
pub async fn logout(
    auth: AuthUser,
    State(db): State<PgPool>,
    jar: CookieJar,
) -> Result<impl IntoResponse> {
    let mut conn = db.acquire().await?;
    db::delete_session(&mut conn, auth.session_id).await?;

    Ok((
        StatusCode::NO_CONTENT,
        jar.remove(session::removal_cookie()),
    ))
}

/// `GET /api/user`: the logged-in user.
pub async fn current_user(auth: AuthUser, State(db): State<PgPool>) -> Result<Json<db::User>> {
    let mut conn = db.acquire().await?;

    let user = db::find_user(&mut conn, auth.user_id)
        .await?
        .ok_or(Error::NotFound)?;

    Ok(Json(user))
}
