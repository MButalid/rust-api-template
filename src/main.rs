use anyhow::Context;
use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use template::{config::Config, http};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // This returns an error if the `.env` file doesn't exists, but that's not what we want
    dotenv::dotenv().ok();

    // Log level is controlled by `RUST_LOG`, e.g. `RUST_LOG=template=debug,tower_http=debug`.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "template=info,tower_http=info".into()),
        )
        .init();

    // Parse our configuration from the environment.
    // This will exit with a help message if something is wrong.
    let config = Config::parse();

    // I create a single connection pool for SQLx that's shared across the whole application.
    // This saves me from opening a new connection for every API call, which is wasteful.
    let db = PgPoolOptions::new()
        .max_connections(50)
        .connect(&config.database_url)
        .await
        .context("could not connect to database_url")?;

    sqlx::migrate!().run(&db).await?;

    // Spin up our API.
    http::serve(config, db).await?;
    Ok(())
}
