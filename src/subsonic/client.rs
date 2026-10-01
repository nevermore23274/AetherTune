use std::fmt;
use std::time::Duration;

use rand::Rng;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::config::SubsonicConfig;
use super::models::*;

/// Oldest API version whose endpoints we use (search3/getArtists/getAlbum
/// arrived in 1.8, token auth in 1.13). Servers accept any version <= theirs.
const API_VERSION: &str = "1.13.0";
/// Identifies us in the server's "now playing"/client list.
const CLIENT_NAME: &str = "AetherTune";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum SubsonicError {
    /// No server URL or username has been configured.
    NotConfigured,
    InvalidUrl(String),
    /// Network-level failure (DNS, refused, timeout, TLS, non-2xx status).
    Http(reqwest::Error),
    /// The server answered with status="failed".
    Api { code: u32, message: String },
    /// The server answered but not with a recognizable Subsonic response.
    BadResponse(String),
}

impl fmt::Display for SubsonicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "Subsonic server URL and username are not set"),
            Self::InvalidUrl(url) => write!(f, "Invalid Subsonic server URL: {}", url),
            Self::Http(e) if e.is_timeout() => write!(f, "Server did not respond within {}s", REQUEST_TIMEOUT.as_secs()),
            Self::Http(e) if e.is_connect() => write!(f, "Could not connect to server: {}", root_cause(e)),
            Self::Http(e) => write!(f, "HTTP error: {}", e),
            Self::Api { code: 40, .. } => write!(f, "Wrong username or password"),
            Self::Api { code: 41, .. } => write!(
                f,
                "Server does not support token authentication for this user (e.g. LDAP accounts)"
            ),
            Self::Api { code, message } => write!(f, "Server error {}: {}", code, message),
            Self::BadResponse(msg) => write!(f, "Unexpected server response: {}", msg),
        }
    }
}

impl std::error::Error for SubsonicError {}

/// reqwest's own message for connect failures is just "error sending
/// request"; the useful part ("Connection refused", "dns error: ...") is
/// the innermost source.
fn root_cause(e: &(dyn std::error::Error + 'static)) -> String {
    let mut current = e;
    while let Some(source) = current.source() {
        current = source;
    }
    current.to_string()
}

impl From<reqwest::Error> for SubsonicError {
    fn from(e: reqwest::Error) -> Self {
        // Drop the request URL from the error — it carries the auth token
        // and salt, which would otherwise end up in printed messages.
        Self::Http(e.without_url())
    }
}

pub type Result<T> = std::result::Result<T, SubsonicError>;

#[derive(Clone)]
pub struct SubsonicClient {
    http: reqwest::Client,
    /// Server root with any trailing "/" or "/rest" removed.
    base_url: String,
    username: String,
    password: String,
}

impl SubsonicClient {
    pub fn new(config: &SubsonicConfig) -> Result<Self> {
        if !config.is_complete() {
            return Err(SubsonicError::NotConfigured);
        }
        let base_url = normalize_base_url(&config.server_url);
        // Validate up front so later URL building can't fail.
        Url::parse(&format!("{}/rest/ping.view", base_url))
            .map_err(|_| SubsonicError::InvalidUrl(config.server_url.clone()))?;

        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        Ok(Self {
            http,
            base_url,
            username: config.username.trim().to_string(),
            password: config.password.clone(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Checks that the server is reachable and the credentials are valid.
    pub async fn ping(&self) -> Result<ServerInfo> {
        let response = self.call("ping", &[]).await?;
        let field = |key: &str| response.get(key).and_then(Value::as_str).map(str::to_string);
        Ok(ServerInfo {
            api_version: field("version").unwrap_or_default(),
            server_name: field("type"),
            server_version: field("serverVersion"),
        })
    }

    /// All artists in the library (ID3 tag based), flattened across the
    /// server's alphabetical index groups.
    pub async fn get_artists(&self) -> Result<Vec<Artist>> {
        let payload: ArtistsPayload = self.call_payload("getArtists", &[], "artists").await?;
        Ok(payload.index.into_iter().flat_map(|i| i.artist).collect())
    }

    pub async fn get_artist_albums(&self, artist_id: &str) -> Result<Vec<Album>> {
        let payload: ArtistWithAlbums = self.call_payload("getArtist", &[("id", artist_id)], "artist").await?;
        Ok(payload.album)
    }

    pub async fn get_album_songs(&self, album_id: &str) -> Result<Vec<Song>> {
        let payload: AlbumWithSongs = self.call_payload("getAlbum", &[("id", album_id)], "album").await?;
        Ok(payload.song)
    }

    /// `count` random albums from the library (getAlbumList2, type=random).
    pub async fn get_random_albums(&self, count: u32) -> Result<Vec<Album>> {
        self.get_album_list("random", count, 0).await
    }

    /// A page of the album list (getAlbumList2). `kind` is the Subsonic list
    /// type, e.g. "alphabeticalByName", "newest", "random".
    pub async fn get_album_list(&self, kind: &str, size: u32, offset: u32) -> Result<Vec<Album>> {
        let size = size.to_string();
        let offset = offset.to_string();
        let payload: AlbumList2 = self
            .call_payload(
                "getAlbumList2",
                &[("type", kind), ("size", &size), ("offset", &offset)],
                "albumList2",
            )
            .await?;
        Ok(payload.album)
    }

    pub async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let payload: PlaylistsPayload = self.call_payload("getPlaylists", &[], "playlists").await?;
        Ok(payload.playlist)
    }

    pub async fn get_playlist_songs(&self, playlist_id: &str) -> Result<Vec<Song>> {
        let payload: PlaylistWithSongs = self.call_payload("getPlaylist", &[("id", playlist_id)], "playlist").await?;
        Ok(payload.entry)
    }

    pub async fn search(&self, query: &str) -> Result<SearchResults> {
        let payload: SearchResult3 = self
            .call_payload(
                "search3",
                &[("query", query), ("artistCount", "20"), ("albumCount", "20"), ("songCount", "50")],
                "searchResult3",
            )
            .await?;
        Ok(SearchResults {
            artists: payload.artist,
            albums: payload.album,
            songs: payload.song,
        })
    }

    /// URL mpv can play directly. Contains a salted auth token, so it must
    /// never be persisted (history/favorites should store the song ID and
    /// call this again at play time).
    pub fn stream_url(&self, song_id: &str) -> String {
        self.endpoint_url("stream", &[("id", song_id)]).to_string()
    }

    fn endpoint_url(&self, endpoint: &str, params: &[(&str, &str)]) -> Url {
        let salt = format!("{:016x}", rand::rng().random::<u64>());
        let token = auth_token(&self.password, &salt);
        let auth = [
            ("u", self.username.as_str()),
            ("t", token.as_str()),
            ("s", salt.as_str()),
            ("v", API_VERSION),
            ("c", CLIENT_NAME),
            ("f", "json"),
        ];
        let url = format!("{}/rest/{}.view", self.base_url, endpoint);
        // Can't fail: base_url was validated in new() and endpoint is ASCII.
        Url::parse_with_params(&url, auth.iter().chain(params.iter())).expect("valid Subsonic URL")
    }

    /// Calls an endpoint and returns the inner "subsonic-response" object,
    /// turning status="failed" into SubsonicError::Api.
    async fn call(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Value> {
        let response = self
            .http
            .get(self.endpoint_url(endpoint, params))
            .send()
            .await?
            .error_for_status()?;
        let body: Value = response
            .json()
            .await
            .map_err(|_| SubsonicError::BadResponse("not JSON — is this a Subsonic server?".to_string()))?;
        unwrap_envelope(body)
    }

    async fn call_payload<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
        key: &str,
    ) -> Result<T> {
        let mut response = self.call(endpoint, params).await?;
        extract_payload(&mut response, key)
    }
}

fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    trimmed.strip_suffix("/rest").unwrap_or(trimmed).to_string()
}

/// Subsonic token auth: hex(md5(password + salt)).
fn auth_token(password: &str, salt: &str) -> String {
    format!("{:x}", md5::compute(format!("{}{}", password, salt)))
}

fn unwrap_envelope(mut body: Value) -> Result<Value> {
    let response = body
        .get_mut("subsonic-response")
        .map(Value::take)
        .ok_or_else(|| SubsonicError::BadResponse("missing \"subsonic-response\"".to_string()))?;

    match response.get("status").and_then(Value::as_str) {
        Some("ok") => Ok(response),
        Some("failed") => {
            let error = response.get("error");
            Err(SubsonicError::Api {
                code: error.and_then(|e| e.get("code")).and_then(Value::as_u64).unwrap_or(0) as u32,
                message: error
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string(),
            })
        }
        _ => Err(SubsonicError::BadResponse("missing status".to_string())),
    }
}

fn extract_payload<T: DeserializeOwned>(response: &mut Value, key: &str) -> Result<T> {
    let payload = response
        .get_mut(key)
        .map(Value::take)
        .ok_or_else(|| SubsonicError::BadResponse(format!("missing \"{}\"", key)))?;
    serde_json::from_value(payload).map_err(|e| SubsonicError::BadResponse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn client() -> SubsonicClient {
        SubsonicClient::new(&SubsonicConfig {
            server_url: "http://music.local:4533/rest/".to_string(),
            username: "me".to_string(),
            password: "sesame".to_string(),
        })
        .unwrap()
    }

    #[test]
    fn token_matches_spec_example() {
        // Example from the Subsonic API docs.
        assert_eq!(auth_token("sesame", "c19b2d"), "26719a1196d2a940705a59634eb18eab");
    }

    #[test]
    fn base_url_is_normalized() {
        assert_eq!(normalize_base_url(" http://h:4533/ "), "http://h:4533");
        assert_eq!(normalize_base_url("http://h:4533/rest"), "http://h:4533");
        assert_eq!(normalize_base_url("https://h/music/rest/"), "https://h/music");
        assert_eq!(client().base_url(), "http://music.local:4533");
    }

    #[test]
    fn incomplete_or_invalid_config_is_rejected() {
        let empty = SubsonicConfig::default();
        assert!(matches!(SubsonicClient::new(&empty), Err(SubsonicError::NotConfigured)));

        let bad = SubsonicConfig {
            server_url: "not a url".to_string(),
            username: "me".to_string(),
            password: String::new(),
        };
        assert!(matches!(SubsonicClient::new(&bad), Err(SubsonicError::InvalidUrl(_))));
    }

    #[test]
    fn stream_url_carries_token_not_password() {
        let url = Url::parse(&client().stream_url("song 1")).unwrap();
        assert_eq!(url.path(), "/rest/stream.view");
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["id"], "song 1");
        assert_eq!(params["u"], "me");
        assert_eq!(params["t"], auth_token("sesame", &params["s"]));
        assert!(!params.contains_key("p"));
        assert!(!url.as_str().contains("sesame"));
    }

    #[test]
    fn salt_is_fresh_per_request() {
        let c = client();
        assert_ne!(c.stream_url("1"), c.stream_url("1"));
    }

    #[test]
    fn failed_envelope_becomes_api_error() {
        let body = json!({ "subsonic-response": {
            "status": "failed", "version": "1.16.1",
            "error": { "code": 40, "message": "Wrong username or password" }
        }});
        match unwrap_envelope(body) {
            Err(SubsonicError::Api { code, .. }) => assert_eq!(code, 40),
            other => panic!("expected Api error, got {:?}", other),
        }
    }

    #[test]
    fn non_subsonic_json_is_bad_response() {
        assert!(matches!(unwrap_envelope(json!({ "hello": 1 })), Err(SubsonicError::BadResponse(_))));
    }

    #[test]
    fn parses_artists_across_index_groups() {
        let mut response = unwrap_envelope(json!({ "subsonic-response": {
            "status": "ok", "version": "1.16.1",
            "artists": { "index": [
                { "name": "A", "artist": [{ "id": "1", "name": "Air", "albumCount": 3 }] },
                { "name": "B", "artist": [{ "id": 2, "name": "Björk" }] }
            ]}
        }}))
        .unwrap();
        let payload: ArtistsPayload = extract_payload(&mut response, "artists").unwrap();
        let artists: Vec<Artist> = payload.index.into_iter().flat_map(|i| i.artist).collect();
        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].album_count, Some(3));
        // Numeric IDs from older servers are accepted as strings.
        assert_eq!(artists[1].id, "2");
    }

    #[test]
    fn parses_album_songs_with_missing_optional_fields() {
        let mut response = unwrap_envelope(json!({ "subsonic-response": {
            "status": "ok", "version": "1.16.1",
            "album": { "id": "al1", "name": "OK Computer", "song": [
                { "id": "s1", "title": "Airbag", "track": 1, "duration": 284,
                  "artist": "Radiohead", "suffix": "flac", "bitRate": 900, "albumId": "al1" },
                { "id": "s2", "title": "Paranoid Android" }
            ]}
        }}))
        .unwrap();
        let album: AlbumWithSongs = extract_payload(&mut response, "album").unwrap();
        assert_eq!(album.song.len(), 2);
        assert_eq!(album.song[0].bit_rate, Some(900));
        assert_eq!(album.song[0].album_id.as_deref(), Some("al1"));
        assert_eq!(album.song[1].duration, None);
    }

    #[test]
    fn parses_playlists_and_playlist_entries() {
        let mut response = unwrap_envelope(json!({ "subsonic-response": {
            "status": "ok", "version": "1.16.1",
            "playlists": { "playlist": [{ "id": "p1", "name": "Road trip", "songCount": 2, "owner": "me" }] }
        }}))
        .unwrap();
        let lists: PlaylistsPayload = extract_payload(&mut response, "playlists").unwrap();
        assert_eq!(lists.playlist[0].song_count, Some(2));

        let mut response = unwrap_envelope(json!({ "subsonic-response": {
            "status": "ok", "version": "1.16.1",
            "playlist": { "id": "p1", "name": "Road trip", "entry": [{ "id": "s1", "title": "Airbag" }] }
        }}))
        .unwrap();
        let list: PlaylistWithSongs = extract_payload(&mut response, "playlist").unwrap();
        assert_eq!(list.entry[0].title, "Airbag");
    }

    #[test]
    fn empty_search_has_no_arrays() {
        let mut response = unwrap_envelope(json!({ "subsonic-response": {
            "status": "ok", "version": "1.16.1", "searchResult3": {}
        }}))
        .unwrap();
        let result: SearchResult3 = extract_payload(&mut response, "searchResult3").unwrap();
        assert!(result.artist.is_empty() && result.album.is_empty() && result.song.is_empty());
    }
}
