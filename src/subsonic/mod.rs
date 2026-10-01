// ── Subsonic ──────────────────────────────────────────────────────────
//
// Client for personal music servers that speak the Subsonic REST API
// (Subsonic, Navidrome, Gonic, Airsonic, ...). Deliberately kept apart
// from the radio code in `core/` so changes to one source can't break
// the other — the only thing the two will share is mpv playback.
//
//   config.rs — server URL + credentials (persisted in config.json)
//   client.rs   — authenticated REST calls and stream URL generation
//   models.rs   — response types (artists, albums, songs)
//   playback.rs — song queue played through the shared mpv player
//   session.rs  — browser state, background fetches, and the play queue
//                 as held by App (the Subsonic counterpart of its radio fields)

pub mod client;
pub mod config;
pub mod models;
pub mod playback;
pub mod session;

use client::{SubsonicClient, SubsonicError};
use config::SubsonicConfig;
use models::ServerInfo;

/// Builds a client from the given settings and pings the server.
pub async fn test_connection(config: &SubsonicConfig) -> Result<ServerInfo, SubsonicError> {
    SubsonicClient::new(config)?.ping().await
}

/// One-line summary of a successful ping, e.g. "navidrome 0.64.2".
pub fn describe_server(info: &ServerInfo) -> String {
    match &info.server_name {
        Some(name) => format!("{} {}", name, info.server_version.as_deref().unwrap_or("")).trim_end().to_string(),
        None => format!("Subsonic API {}", info.api_version),
    }
}

/// Entry point for `aethertune --subsonic-test`: pings the server saved in
/// config.json and prints the result. Runs before the TUI starts, so plain
/// println! is fine. Returns the process exit code.
pub async fn run_connection_test() -> i32 {
    let config = crate::storage::config::Config::load().subsonic;
    if !config.is_complete() {
        eprintln!("No Subsonic server configured — set one in the launcher's Settings screen.");
        return 1;
    }

    println!("Connecting to {} as {} ...", config.server_url.trim(), config.username.trim());
    match test_connection(&config).await {
        Ok(info) => {
            println!("✓ Connected — {} (API {})", describe_server(&info), info.api_version);
            0
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            1
        }
    }
}
