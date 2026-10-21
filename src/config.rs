use std::net::SocketAddr;

/// The configuration parameters for the application.
///
/// Can be passed on the command line, or pulled from environment variables.
///
/// This is a pretty simple configuration struct as far as backend APIs go. You could imagine
/// a bunch of other parameters going here, like API keys for external services
/// or flags enabling or disabling certain features or test modes of the API.
///
/// Every field is `pub` so tests can build a `Config` directly instead of going through the environment.
///
/// See `.env.sample` in the repository root for details.
#[derive(clap::Parser, Clone, Debug)]
pub struct Config {
    #[clap(long, env)]
    pub database_url: String,

    /// The address the HTTP server listens on.
    #[clap(long, env, default_value = "0.0.0.0:8000")]
    pub bind_address: SocketAddr,

    /// Set the `Secure` attribute on the session cookie. Leave this off for local HTTP.
    #[clap(long, env, default_value_t = false)]
    pub cookie_secure: bool,
}
