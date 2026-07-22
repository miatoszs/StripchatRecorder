//! Stripchat Recorder Library Crate Root
//!
//! Server （HTTP API + SSE），。
//! Only supports Server mode (HTTP API + SSE); listen port is specified via CLI arg or env var.

pub mod commands;
pub mod config;
pub mod core;
pub mod locale;
pub mod postprocess;
pub mod recording;
pub mod relay;
pub mod server_mod;
pub mod streaming;
pub mod watcher;

/// ：， HTTP Server 。
///
/// Port resolution order:
/// 1. First CLI argument (e.g. `./stripchat-recorder 3030`)
/// 2. `PORT` environment variable
/// 3. `server_port` field in `config/settings.json`
/// 4. Default: 3030
pub fn run() {
    // ：CLI  >  >  >
    // Resolve port: CLI arg > env var > config file > default
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("PORT").ok().and_then(|s| s.parse().ok()))
        .or_else(|| {
            config::settings::AppState::new()
                .ok()
                .map(|s| s.get_settings().server_port)
        })
        .unwrap_or(3030);

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(server_mod::server::run_server(port));
}
