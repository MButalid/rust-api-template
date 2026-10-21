# Architecture

An Axum + SQLx + Postgres JSON API. The code is the main documentation; this page is the short version of the conventions.

## Getting started

```sh
cp .env.sample .env
docker compose up -d          # Postgres 18 (needed for uuidv7())
cargo sqlx migrate run
cargo run
cargo test                    # each #[sqlx::test] gets its own throwaway database
```

## Layout

```
src/
  main.rs          tracing, config, pool, migrations, serve
  config.rs        all configuration (clap + env)
  http/            shared plumbing only, no features
    mod.rs         AppState, app() router assembly, serve() with graceful shutdown
    error.rs       Error -> HTTP response, on_constraint helper
  auth/            one folder per feature
    mod.rs         router() + extractors other features use (AuthUser, MaybeAuthUser)
    handlers.rs    HTTP: extract, validate, call db, respond
    db.rs          SQL: plain functions taking &mut PgConnection
    password.rs    helpers that are neither HTTP nor SQL
    session.rs
tests/
  common/mod.rs    app(), send(), register() helpers
  auth.rs
```

## Rules

1. **Organise by feature, not by layer.** A feature owns its handlers, queries and types. `http/` only holds what every feature shares.
2. **Handlers are thin.** They extract input, validate it (`req.validate().map_err(Error::validation_errors)?`), call `db` functions, and build the response. No SQL in handlers.
3. **Query functions take `&mut PgConnection`.** The same function works with `&mut conn` from `pool.acquire()` or `&mut tx` inside a transaction, so multi-step writes compose without extra plumbing. Return `sqlx::Result` and let the handler decide which errors become client errors (`.on_constraint("table_column_key", ...)`).
4. **No repository traits.** Tests run against a real Postgres, so there is nothing to mock. Use `query!`/`query_as!` so the compiler checks the SQL.
5. **State through `State<T>`.** Add fields to `AppState`; handlers extract only what they need (`State<PgPool>`, `State<Arc<Config>>`) via `FromRef`.
6. **Errors go through `http::Error`.** Client errors carry a safe message. `Sqlx`/`Anyhow` errors are logged in full and returned as a generic 500.
7. **Slow CPU work goes in `spawn_blocking`** (see `auth/password.rs`), and happens before opening a transaction.

## Adding a feature

1. Add a migration: `cargo sqlx migrate add -r <name>`. Use `select trigger_updated_at('<table>');` for tables with `updated_at`.
2. Create `src/<feature>/` with `mod.rs` (`pub fn router() -> Router<AppState>`), `handlers.rs` and `db.rs`.
3. Register it in `src/lib.rs` and nest its router in `http::app`.
4. Use `AuthUser` (required login) or `MaybeAuthUser` (optional login) as a handler argument. Return `Error::Forbidden` when a logged-in user touches something they don't own.
5. Add `tests/<feature>.rs` using the `common` helpers.
6. Run `cargo sqlx prepare -- --all-targets` and commit `.sqlx/`.

## Auth

- `POST /api/users` registers and logs in, `POST /api/sessions` logs in, `DELETE /api/sessions` logs out, and `GET /api/user` returns the current user.
- Sessions are opaque random tokens in an `HttpOnly`, `SameSite=Lax` cookie. Only the SHA-256 of the token is stored. Expiry is 30 days and slides forward (written at most once a day). Expired rows are cleaned up on the user's next login.
- Passwords use Argon2id with default parameters.

**Removing auth:** delete `src/auth/`, the `pub mod auth;` in `lib.rs`, the `.nest("/api", auth::router())` line, the `user` and `session` migrations, and `tests/auth.rs`. Then drop the unused dependencies (`argon2`, `sha2`, `rand`, `base64`, `axum-extra`).

## CI

Set `SQLX_OFFLINE=true` to build without a database using the committed `.sqlx/` data. Tests still need a Postgres 18 `DATABASE_URL`.
