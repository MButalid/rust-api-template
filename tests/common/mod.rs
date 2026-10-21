//! Shared helpers for HTTP-level tests.
//!
//! Tests drive the real router with `tower::ServiceExt::oneshot`, no TCP involved, against a fresh migrated
//! database that `#[sqlx::test]` creates per test. Nothing is mocked.

#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use template::config::Config;
use tower::ServiceExt;

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// The parsed JSON body, or `Value::Null` if the body was empty or not JSON.
    pub body: Value,
}

impl TestResponse {
    /// The `name=value` pair of the session cookie, ready to send back in a `Cookie` header.
    pub fn session_cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("session="))
            .and_then(|v| v.split(';').next())
            .map(str::to_owned)
    }
}

pub fn app(db: PgPool) -> Router {
    let config = Config {
        database_url: String::new(),
        bind_address: "127.0.0.1:0".parse().unwrap(),
        cookie_secure: false,
    };
    template::http::app(config, db)
}

pub async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> TestResponse {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }

    let req = match body {
        Some(body) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    TestResponse {
        status,
        headers,
        body,
    }
}

/// Registers `username` (with email `<username>@example.com` and password `password123`) and returns
/// the session cookie.
pub async fn register(app: &Router, username: &str) -> String {
    let res = send(
        app,
        Method::POST,
        "/api/users",
        Some(json!({
            "username": username,
            "email": format!("{username}@example.com"),
            "password": "password123",
        })),
        None,
    )
    .await;

    assert_eq!(
        res.status,
        StatusCode::CREATED,
        "register failed: {}",
        res.body
    );
    res.session_cookie()
        .expect("register should set a session cookie")
}
