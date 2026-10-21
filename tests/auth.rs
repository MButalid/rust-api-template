mod common;

use axum::http::{Method, StatusCode};
use common::{app, register, send};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn register_returns_user_and_logs_in(db: PgPool) {
    let app = app(db);
    let cookie = register(&app, "alice").await;

    let res = send(&app, Method::GET, "/api/user", None, Some(&cookie)).await;

    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["username"], "alice");
    assert_eq!(res.body["email"], "alice@example.com");
    assert!(res.body.get("password_hash").is_none());
}

#[sqlx::test]
async fn register_rejects_invalid_input(db: PgPool) {
    let app = app(db);

    let res = send(
        &app,
        Method::POST,
        "/api/users",
        Some(json!({ "username": "", "email": "nope", "password": "short" })),
        None,
    )
    .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    for field in ["username", "email", "password"] {
        assert!(
            res.body["errors"][field].is_array(),
            "missing error for {field}: {}",
            res.body
        );
    }
}

#[sqlx::test]
async fn register_rejects_duplicate_email_case_insensitively(db: PgPool) {
    let app = app(db);
    register(&app, "alice").await;

    let res = send(
        &app,
        Method::POST,
        "/api/users",
        Some(json!({ "username": "bob", "email": "ALICE@example.com", "password": "password123" })),
        None,
    )
    .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["errors"]["email"][0], "email taken");
}

#[sqlx::test]
async fn login_with_correct_password(db: PgPool) {
    let app = app(db);
    register(&app, "alice").await;

    let res = send(
        &app,
        Method::POST,
        "/api/sessions",
        Some(json!({ "email": "alice@example.com", "password": "password123" })),
        None,
    )
    .await;

    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["username"], "alice");
    let cookie = res
        .session_cookie()
        .expect("login should set a session cookie");

    let res = send(&app, Method::GET, "/api/user", None, Some(&cookie)).await;
    assert_eq!(res.status, StatusCode::OK);
}

#[sqlx::test]
async fn login_failures_are_indistinguishable(db: PgPool) {
    let app = app(db);
    register(&app, "alice").await;

    let wrong_password = send(
        &app,
        Method::POST,
        "/api/sessions",
        Some(json!({ "email": "alice@example.com", "password": "wrong-password" })),
        None,
    )
    .await;
    let unknown_email = send(
        &app,
        Method::POST,
        "/api/sessions",
        Some(json!({ "email": "nobody@example.com", "password": "password123" })),
        None,
    )
    .await;

    assert_eq!(wrong_password.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_email.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password.body, unknown_email.body);
    assert!(wrong_password.session_cookie().is_none());
}

#[sqlx::test]
async fn current_user_requires_a_valid_session(db: PgPool) {
    let app = app(db);

    let missing = send(&app, Method::GET, "/api/user", None, None).await;
    let bogus = send(
        &app,
        Method::GET,
        "/api/user",
        None,
        Some("session=not-a-real-token"),
    )
    .await;

    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(bogus.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn logout_invalidates_the_session(db: PgPool) {
    let app = app(db);
    let cookie = register(&app, "alice").await;

    let res = send(&app, Method::DELETE, "/api/sessions", None, Some(&cookie)).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // Even if a client keeps sending the old cookie, it no longer works.
    let res = send(&app, Method::GET, "/api/user", None, Some(&cookie)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn expired_sessions_are_rejected(db: PgPool) {
    let app = app(db.clone());
    let cookie = register(&app, "alice").await;

    sqlx::query("update session set expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();

    let res = send(&app, Method::GET, "/api/user", None, Some(&cookie)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
}
