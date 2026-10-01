/// Subsonic server connection settings. Persisted as the "subsonic" object
/// in config.json (see storage::config) and edited from the launcher's
/// Settings screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubsonicConfig {
    /// Server root, e.g. "http://192.168.1.10:4533". A trailing "/rest"
    /// is tolerated and stripped by the client.
    pub server_url: String,
    pub username: String,
    /// Stored as plain text: Subsonic's token auth is md5(password + salt)
    /// with a fresh salt per request, so the client needs the real password.
    pub password: String,
}

impl SubsonicConfig {
    pub fn is_complete(&self) -> bool {
        !self.server_url.trim().is_empty() && !self.username.trim().is_empty()
    }
}
