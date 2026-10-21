# Rust API template

My starting point for new backend projects: an Axum + SQLx + Postgres JSON API with cookie-session auth, real-database integration tests, and the conventions already decided.

## What's included

- **Axum 0.8** with typed shared state (`State<AppState>` + `FromRef`), request tracing, and graceful shutdown.
- **SQLx + Postgres 18** with compile-time checked queries, migrations run on startup, and committed offline query data (`.sqlx/`).
- **Auth**: register, login, logout and current user. Argon2id passwords, opaque session tokens stored hashed in Postgres, `HttpOnly` cookie, 30-day sliding expiry. `AuthUser` / `MaybeAuthUser` extractors for other features.
- **Error handling**: one `Error` type mapping to HTTP responses, `422` field errors from `validator` and from unique constraints, and logging of every 500.
- **Tests**: `#[sqlx::test]` gives each test a fresh migrated database; tests drive the real router with no mocks.
- **Database conventions**: `created_at`/`updated_at` with a one-line trigger helper, a case-insensitive collation for usernames and emails, UUIDv7 primary keys.

## Quick start

Requires Rust, Docker, and `sqlx-cli` (`cargo install sqlx-cli --no-default-features --features postgres,native-tls`).

```sh
cp .env.sample .env
docker compose up -d
cargo sqlx migrate run
cargo run          # http://localhost:8000
cargo test
```

If port 5432 is taken, run `POSTGRES_PORT=5433 docker compose up -d` and change the port in `DATABASE_URL`.

## API

| Method   | Path            | Description                          |
| -------- | --------------- | ------------------------------------ |
| `POST`   | `/api/users`    | Register (`username`, `email`, `password`) and log in |
| `POST`   | `/api/sessions` | Log in (`email`, `password`)         |
| `DELETE` | `/api/sessions` | Log out                              |
| `GET`    | `/api/user`     | Current user                         |

## Starting a new project from this

1. Copy the repo and rename the package in `Cargo.toml` (and the `template=` log filter in `main.rs` and `.env.sample`).
2. Change `POSTGRES_DB` in `docker-compose.yml` and the database name in `DATABASE_URL`.
3. Keep or remove auth. Removal steps are in [ARCHITECTURE.md](ARCHITECTURE.md#auth).
4. Add features following the checklist in [ARCHITECTURE.md](ARCHITECTURE.md#adding-a-feature).

## Configuration

All settings come from environment variables (or `.env`) and can also be passed as CLI flags (`cargo run -- --help`).

| Variable        | Default        | Purpose                                     |
| --------------- | -------------- | ------------------------------------------- |
| `DATABASE_URL`  | (required)     | Postgres connection string                  |
| `BIND_ADDRESS`  | `0.0.0.0:8000` | Address the server listens on               |
| `COOKIE_SECURE` | `false`        | Mark the session cookie `Secure` (use with HTTPS) |
| `RUST_LOG`      | `template=info,tower_http=info` | Log filter               |
| `SQLX_OFFLINE`  | unset          | Build from `.sqlx/` without a database      |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the project layout and coding conventions.
