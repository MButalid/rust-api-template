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

## Orientation: a walk through the code comments

Most of the reasoning behind this template is written as comments next to the code it explains. If you're new to the project (or it's me, six months later), read the files in this order. Each step says what to look for.

### 1. Startup: [`src/main.rs`](src/main.rs)

This file shows the whole boot sequence top to bottom: load `.env`, set up logging, parse config, connect to the database, run migrations, serve.

- `dotenv().ok()` deliberately ignores a missing `.env`. In production, configuration comes from real environment variables, so a missing file isn't an error.
- There's a **single shared connection pool** for the whole application instead of a connection per request, because opening connections is wasteful.
- Migrations run on startup, so the schema always matches the code that's running.

### 2. Configuration: [`src/config.rs`](src/config.rs)

All configuration lives in one `clap` struct. Each field can be a CLI flag or an environment variable, and a missing value exits with a help message instead of failing somewhere deep in the app. The comments point out what typically gets added here later: API keys, feature flags, test modes. Fields are `pub` so tests can build a `Config` directly.

### 3. Shared HTTP setup: [`src/http/`](src/http)

- [`mod.rs`](src/http/mod.rs) explains `AppState`. Handlers ask for only the piece they need (`State<PgPool>`, `State<Arc<Config>>`), and missing state is a compile error rather than a runtime 500. It's also the **only** place features are wired into the router.
- [`error.rs`](src/http/error.rs) defines the one `Error` type every handler returns. Client errors carry a message that's safe to show. Database and internal errors are logged in full but return a generic 500, so internals never leak. `on_constraint` turns a specific unique-constraint violation into a friendly `422`.

### 4. A feature, end to end: [`src/auth/`](src/auth)

[`auth/mod.rs`](src/auth/mod.rs) opens with the layout every feature copies. HTTP code goes in `handlers.rs`, SQL goes in `db.rs`, and helpers that are neither go in their own files. After that, read:

- [`handlers.rs`](src/auth/handlers.rs) shows the pattern for a request: validate, do slow work (password hashing) *before* opening a transaction, create the user and session atomically, and map constraint errors to field errors. Login failures always return the same message so the endpoint can't be used to find out whether an email has an account.
- [`db.rs`](src/auth/db.rs) explains why query functions take `&mut PgConnection`: the same function works on a pooled connection or inside a transaction.
- [`session.rs`](src/auth/session.rs) and [`password.rs`](src/auth/password.rs) cover why only a hash of the token is stored, why expiry is extended at most once a day, why the cookie lifetime is longer than the session's, and why hashing runs on the blocking thread pool.

### 5. Database conventions: [`migrations/`](migrations)

The migrations carry the most opinionated comments in the project.

- **[`setup.sql`](migrations/20261021062631_setup.sql)** is boilerplate reused in nearly every project:
  - Every table gets `created_at` and `updated_at`, which help with debugging and auditing. Keeping `updated_at` current normally needs a trigger per table, so the helper reduces it to one line: `select trigger_updated_at('<table>');`.
  - A `case_insensitive` collation makes `unique` on usernames and emails ignore case without remembering to lowercase anything in code.
- **[`user.up.sql`](migrations/20261021062705_user.up.sql)** shows the table style:
  - **Singular table names** (`user`, `session`), because queries read better.
  - **`"user"` is quoted** because `user` is a reserved word in Postgres.
  - **Primary keys include the table name** (`user_id`, not `id`). Joins can then be written as `inner join article using (user_id)`, and it's always obvious which key is which.
  - UUIDv7 primary keys, which are sortable by creation time.
  - The comments note what the case-insensitive collation *doesn't* support: `like`/`ilike` searches need a second index using the default collation.
  - Some rules are deliberately enforced in code review instead of with triggers, such as "`created_at` never changes", because a trigger would be overkill.
- **[`session.up.sql`](migrations/20261021070000_session.up.sql)** applies the same conventions to a new table. It also explains why only a token *hash* is stored, and that deleting a user logs them out everywhere.

### Why the SQL is lowercase

All SQL in this project, in both migrations and `query!` calls, is written in lowercase:

```sql
select user_id, username from "user" where email = $1
```

rather than:

```sql
SELECT user_id, username FROM "user" WHERE email = $1
```

The reasoning, from the top of `setup.sql`:

- **It's easier to read.** Lowercase is friendlier on the eyes, and you're not being shouted at all day.
- **Uppercase keywords are a habit left over from before syntax highlighting.** Case used to be the only way to tell keywords apart from table and column names. Editors now colour keywords, so the convention survives only out of habit.
- **SQL doesn't care.** Postgres keywords and unquoted identifiers are case-insensitive, so writing in lowercase changes nothing at runtime. The only case-sensitive names are quoted identifiers like `"user"`.

That's why it's lowercase *in most cases* rather than all. The exceptions are:

- **Dynamic SQL inside a string.** The `trigger_updated_at` helper builds `CREATE TRIGGER ... EXECUTE FUNCTION ...` with `format()`. That text is a string passed to `execute`, so the editor doesn't highlight it either way and its case doesn't matter.
- **A leftover `SELECT trigger_updated_at('"user"')`** at the end of `user.up.sql`. It behaves the same either way; new migrations should use lowercase.

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
