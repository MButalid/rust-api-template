use std::{borrow::Cow, collections::HashMap};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::error::DatabaseError;

/// The error type returned by every handler.
///
/// Client errors carry a message that is safe to show. Server errors (`Sqlx`, `Anyhow`) are logged in full
/// but only a generic message is returned, so internals never leak to the client.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Return `401 Unauthorized`
    #[error("{0}")]
    Unauthorized(&'static str),

    /// Return `403 Forbidden`, for an authenticated user acting on something they don't own.
    #[error("{0}")]
    Forbidden(&'static str),

    /// Return `404 Not Found`
    #[error("request path not found")]
    NotFound,

    /// Return `422 Unprocessable Entity`
    #[error("error in the request body")]
    UnprocessableEntity {
        errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
    },

    /// Return `500 Internal Server Error` on a `sqlx::Error`.
    #[error("an error occurred with the database")]
    Sqlx(#[from] sqlx::Error),

    /// Return `500 Internal Server Error` on a `anyhow::Error`.
    #[error("an internal server error occurred")]
    Anyhow(#[from] anyhow::Error),
}

impl Error {
    /// Build a `422` from `(field, message)` pairs, e.g. `[("email", "email taken")]`.
    pub fn unprocessable_entity<K, V>(errors: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        let mut error_map = HashMap::new();

        for (key, val) in errors {
            error_map
                .entry(key.into())
                .or_insert_with(Vec::new)
                .push(val.into());
        }

        Self::UnprocessableEntity { errors: error_map }
    }

    /// Convert `validator` errors into a `422`, so handlers can write `req.validate().map_err(Error::validation_errors)?`.
    pub fn validation_errors(errors: validator::ValidationErrors) -> Self {
        let pairs = errors.field_errors().into_iter().flat_map(|(field, errs)| {
            errs.iter().map(move |err| {
                let message = err.message.clone().unwrap_or_else(|| err.code.clone());
                let field = if field == "__all__" {
                    err.code.clone()
                } else {
                    field.clone()
                };
                (field, message)
            })
        });

        Self::unprocessable_entity(pairs)
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::UnprocessableEntity { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Sqlx(_) | Self::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::UnprocessableEntity { errors } => {
                #[derive(serde::Serialize)]
                struct Errors {
                    errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
                }

                (StatusCode::UNPROCESSABLE_ENTITY, Json(Errors { errors })).into_response()
            }
            Self::Sqlx(ref e) => {
                tracing::error!("SQLx error: {e:?}");
                (self.status_code(), self.to_string()).into_response()
            }
            Self::Anyhow(ref e) => {
                tracing::error!("generic error: {e:?}");
                (self.status_code(), self.to_string()).into_response()
            }
            _ => (self.status_code(), self.to_string()).into_response(),
        }
    }
}

/// Lets a handler turn a specific database constraint violation into a client error:
///
/// ```ignore
/// db::insert_user(&mut tx, ..)
///     .await
///     .on_constraint("user_email_key", |_| Error::unprocessable_entity([("email", "email taken")]))?;
/// ```
pub trait ResultExt<T> {
    fn on_constraint<F>(self, name: &str, f: F) -> Result<T, Error>
    where
        F: FnOnce(Box<dyn DatabaseError>) -> Error;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<Error>,
{
    fn on_constraint<F>(self, name: &str, map_err: F) -> Result<T, Error>
    where
        F: FnOnce(Box<dyn DatabaseError>) -> Error,
    {
        self.map_err(|e| match e.into() {
            Error::Sqlx(sqlx::Error::Database(dbe)) if dbe.constraint() == Some(name) => {
                map_err(dbe)
            }
            e => e,
        })
    }
}
