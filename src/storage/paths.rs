use std::path::PathBuf;
use std::sync::OnceLock;

/// The directory AetherTune stores `config.json`, `favorites.json`, and
/// `history.json` in. Resolved once at startup by `resolve_and_set` and
/// read by every storage module thereafter via `base_dir()`.
static BASE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Resolves the storage directory and stashes it for the rest of the
/// process. Must be called exactly once, at the very start of `main()`,
/// before any storage module's `load()` runs — those all read `base_dir()`.
///
/// Resolution order:
/// 1. `--config-dir=PATH` command-line flag
/// 2. `AETHERTUNE_CONFIG_DIR` environment variable
/// 3. `$HOME/.aethertune` (`$USERPROFILE/.aethertune` on Windows) — the
///    long-standing default, unchanged unless one of the above is set.
pub fn resolve_and_set(args: &[String]) {
    let from_flag = args
        .iter()
        .find(|a| a.starts_with("--config-dir="))
        .and_then(|a| a.strip_prefix("--config-dir="))
        .map(PathBuf::from);

    let from_env = std::env::var("AETHERTUNE_CONFIG_DIR").ok().map(PathBuf::from);

    let dir = from_flag.or(from_env).unwrap_or_else(default_dir);
    // First call wins; if resolve_and_set is somehow called twice, later
    // calls are silently ignored rather than changing storage mid-session.
    let _ = BASE_DIR.set(dir);
}

fn default_dir() -> PathBuf {
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let mut path = PathBuf::from(base);
    path.push(".aethertune");
    path
}

/// Returns the resolved storage directory, creating it if it doesn't exist
/// yet. Falls back to the default `$HOME/.aethertune` location if
/// `resolve_and_set` was never called (shouldn't happen outside of tests).
pub fn base_dir() -> PathBuf {
    let dir = BASE_DIR.get().cloned().unwrap_or_else(default_dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}