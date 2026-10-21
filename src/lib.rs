///  Contains the shared setup for the API built with Axum: state, errors, layers and `serve`.
///
///  Features don't live here; each one is its own top-level module that exposes a `router()`.
pub mod http;

/// Defines the arguments required to start the server application using [`clap`].
pub mod config;

/// Users and login sessions. Also the reference example of how a feature module is laid out.
///
/// To remove auth from a new project: delete `src/auth/`, the `.nest("/api", auth::router())` line in
/// `http::app`, and the `user`/`session` migrations.
pub mod auth;
