use std::sync::Arc;

use axum::{Router, extract::FromRef};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

use crate::{auth, config::Config};

mod error;

/// Defines a common error type to use for all request handlers.
pub use error::{Error, ResultExt};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The state shared by every request handler.
///
/// Handlers and extractors don't take the whole struct; thanks to `FromRef` they can ask for just the piece
/// they need, e.g. `State(db): State<PgPool>` or `State(config): State<Arc<Config>>`. Forgetting to provide
/// state is a compile error rather than a runtime 500, which is the main reason to prefer this over
/// `Extension`.
///
/// When adding something new (an HTTP client, a cache, an email sender...), add a field here and it
/// becomes extractable everywhere.
#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
}

pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let listener = TcpListener::bind(config.bind_address).await?;
    tracing::info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app(config, db))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Builds the full application router. Tests call this directly instead of [`serve`].
pub fn app(config: Config, db: PgPool) -> Router {
    let state = AppState {
        config: Arc::new(config),
        db,
    };

    // Each feature module exposes its own `router()`; this is the only place they're wired together.
    Router::new()
        .nest("/api", auth::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Resolves on Ctrl-C (or SIGTERM on Unix), letting in-flight requests finish before the server exits.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutting down");
}
