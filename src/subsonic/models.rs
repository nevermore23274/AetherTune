use serde::{Deserialize, Deserializer};

// Response types for the Subsonic JSON API. Only the fields AetherTune
// needs are modelled; everything else in the payload is ignored. Optional
// fields vary between server implementations, so most are Option/default.

/// Result of `ping`, used by the connection test.
#[derive(Clone, Debug)]
pub struct ServerInfo {
    /// Subsonic REST API version the server implements (e.g. "1.16.1").
    pub api_version: String,
    /// OpenSubsonic servers report their name/version (e.g. "navidrome").
    pub server_name: Option<String>,
    pub server_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub album_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    /// `getAlbum`/`search3` use "name"; some older servers send "title".
    #[serde(alias = "title")]
    pub name: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default, deserialize_with = "opt_id_string")]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub song_count: Option<u32>,
    /// Total length in seconds.
    #[serde(default)]
    pub duration: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default, deserialize_with = "opt_id_string")]
    pub album_id: Option<String>,
    #[serde(default)]
    pub track: Option<u32>,
    /// Length in seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    /// Kbps of the original file.
    #[serde(default)]
    pub bit_rate: Option<u32>,
    /// File extension of the original file (e.g. "flac", "mp3").
    #[serde(default)]
    pub suffix: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(deserialize_with = "id_string")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub song_count: Option<u32>,
    /// Total length in seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub owner: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub songs: Vec<Song>,
}

// ── Response envelopes ────────────────────────────────────────────────
// Each endpoint nests its payload under a different key, and servers omit
// empty arrays entirely, hence the #[serde(default)]s.

#[derive(Deserialize)]
pub(crate) struct ArtistsPayload {
    #[serde(default)]
    pub index: Vec<ArtistIndex>,
}

#[derive(Deserialize)]
pub(crate) struct ArtistIndex {
    #[serde(default)]
    pub artist: Vec<Artist>,
}

#[derive(Deserialize)]
pub(crate) struct ArtistWithAlbums {
    #[serde(default)]
    pub album: Vec<Album>,
}

#[derive(Deserialize)]
pub(crate) struct AlbumWithSongs {
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Deserialize)]
pub(crate) struct PlaylistsPayload {
    #[serde(default)]
    pub playlist: Vec<Playlist>,
}

/// getPlaylist calls its songs "entry" rather than "song".
#[derive(Deserialize)]
pub(crate) struct PlaylistWithSongs {
    #[serde(default)]
    pub entry: Vec<Song>,
}

#[derive(Deserialize)]
pub(crate) struct AlbumList2 {
    #[serde(default)]
    pub album: Vec<Album>,
}

#[derive(Deserialize)]
pub(crate) struct SearchResult3 {
    #[serde(default)]
    pub artist: Vec<Artist>,
    #[serde(default)]
    pub album: Vec<Album>,
    #[serde(default)]
    pub song: Vec<Song>,
}

/// IDs are strings in the spec, but some older servers emit bare numbers.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawId {
    Str(String),
    Num(u64),
}

impl From<RawId> for String {
    fn from(raw: RawId) -> String {
        match raw {
            RawId::Str(s) => s,
            RawId::Num(n) => n.to_string(),
        }
    }
}

fn id_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    RawId::deserialize(d).map(String::from)
}

fn opt_id_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<RawId>::deserialize(d).map(|raw| raw.map(String::from))
}
