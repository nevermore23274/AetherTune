use tokio::sync::oneshot;

use crate::audio::player::Player;
use crate::core::types::SongLogEntry;

use super::client::SubsonicClient;
use super::config::SubsonicConfig;
use super::models::{Album, Artist, Playlist, Song};
use super::playback::SubsonicQueue;

/// Albums per page on the Albums tab ("load more" fetches the next page).
const ALBUM_PAGE_SIZE: u32 = 100;
/// How far the seek keys jump.
pub const SEEK_STEP_SECS: f64 = 10.0;

/// Top-level lists in the Subsonic browser, cycled with the panel key.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BrowseTab {
    Artists,
    Albums,
    Playlists,
}

impl BrowseTab {
    pub fn title(self) -> &'static str {
        match self {
            BrowseTab::Artists => "Artists",
            BrowseTab::Albums => "Albums",
            BrowseTab::Playlists => "Playlists",
        }
    }

    fn next(self) -> Self {
        match self {
            BrowseTab::Artists => BrowseTab::Albums,
            BrowseTab::Albums => BrowseTab::Playlists,
            BrowseTab::Playlists => BrowseTab::Artists,
        }
    }
}

/// One row in a browser list.
#[derive(Clone, Debug)]
pub enum Entry {
    Artist(Artist),
    Album(Album),
    Playlist(Playlist),
    Song(Song),
}

/// One screen of the browser: a tab's root list, or something opened from
/// it (an artist's albums, an album's songs, search results, ...).
pub struct Level {
    pub title: String,
    pub entries: Vec<Entry>,
    pub selected: usize,
    /// Offset of the next page, when more can be loaded (Albums tab only).
    pub next_offset: Option<u32>,
    /// Show album track numbers — only meaningful when listing one album,
    /// not in playlists or search results.
    pub numbered: bool,
}

impl Level {
    fn new(title: String, entries: Vec<Entry>, next_offset: Option<u32>) -> Self {
        Self { title, entries, selected: 0, next_offset, numbered: false }
    }
}

/// What to do with a background fetch's entries once they arrive.
enum Request {
    /// Replace the whole stack with this tab's root list.
    Root(BrowseTab),
    /// Open a new level on top of the stack (numbered: an album's songs).
    Open { title: String, numbered: bool },
    /// Append a page to the top level.
    More,
}

struct Loaded {
    request: Request,
    result: Result<(Vec<Entry>, Option<u32>), String>,
}

/// Everything Subsonic-related the app holds: the server client, browser
/// state, and the play queue. Kept separate from the radio fields on App.
pub struct SubsonicSession {
    client: Option<SubsonicClient>,
    /// Why there's no client (not configured / invalid URL), shown in the UI.
    pub setup_error: Option<String>,
    pub tab: BrowseTab,
    pub stack: Vec<Level>,
    pub loading: bool,
    /// Last fetch error, shown in the browser until the next fetch.
    pub error: Option<String>,
    pending: Option<oneshot::Receiver<Loaded>>,
    pub queue: Option<SubsonicQueue>,
    /// Queue index last written to the song log, to log each track once.
    logged_index: Option<usize>,
}

impl SubsonicSession {
    pub fn new(config: &SubsonicConfig) -> Self {
        let (client, setup_error) = match SubsonicClient::new(config) {
            Ok(client) => (Some(client), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self {
            client,
            setup_error,
            tab: BrowseTab::Artists,
            stack: Vec::new(),
            loading: false,
            error: None,
            pending: None,
            queue: None,
            logged_index: None,
        }
    }

    pub fn current_level(&self) -> Option<&Level> {
        self.stack.last()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.current_level().and_then(|level| level.entries.get(level.selected))
    }

    // ── Browsing ──────────────────────────────────────────────────────

    /// Loads the current tab's list the first time the browser is shown.
    pub fn ensure_loaded(&mut self) {
        if self.stack.is_empty() && !self.loading {
            self.load_root(self.tab);
        }
    }

    pub fn cycle_tab(&mut self) {
        if self.loading {
            return;
        }
        self.tab = self.tab.next();
        self.load_root(self.tab);
    }

    pub fn select_next(&mut self) {
        if let Some(level) = self.stack.last_mut() {
            if level.selected + 1 < level.entries.len() {
                level.selected += 1;
            }
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(level) = self.stack.last_mut() {
            level.selected = level.selected.saturating_sub(1);
        }
    }

    /// Returns to the previous level (no-op at a tab's root list).
    pub fn back(&mut self) {
        if self.stack.len() > 1 && !self.loading {
            self.stack.pop();
            self.error = None;
        }
    }

    /// Opens the selected artist/album/playlist. Selected songs are played
    /// by `play_selected` instead; returns false for those.
    pub fn open_selected(&mut self) -> bool {
        if self.loading {
            return false;
        }
        let Some(entry) = self.selected_entry().cloned() else { return false };
        match entry {
            Entry::Artist(artist) => {
                let id = artist.id.clone();
                self.fetch(Request::Open { title: artist.name, numbered: false }, move |client| async move {
                    let albums = client.get_artist_albums(&id).await?;
                    Ok((albums.into_iter().map(Entry::Album).collect(), None))
                });
                true
            }
            Entry::Album(album) => {
                // The breadcrumb already shows the artist when opened from one
                let id = album.id.clone();
                self.fetch(Request::Open { title: album.name, numbered: true }, move |client| async move {
                    let songs = client.get_album_songs(&id).await?;
                    Ok((songs.into_iter().map(Entry::Song).collect(), None))
                });
                true
            }
            Entry::Playlist(playlist) => {
                let id = playlist.id.clone();
                self.fetch(Request::Open { title: playlist.name, numbered: false }, move |client| async move {
                    let songs = client.get_playlist_songs(&id).await?;
                    Ok((songs.into_iter().map(Entry::Song).collect(), None))
                });
                true
            }
            Entry::Song(_) => false,
        }
    }

    /// Plays the songs in the current list starting at the selected one.
    /// Returns false if nothing was started (no song selected, mpv missing).
    pub fn play_selected(&mut self, player: &mut Player, volume: u32) -> bool {
        let Some(client) = self.client.clone() else { return false };
        let Some(level) = self.current_level() else { return false };
        let Some((songs, start)) = songs_from(&level.entries, level.selected) else { return false };

        match SubsonicQueue::play(player, &client, songs, start, volume) {
            Some(queue) => {
                self.queue = Some(queue);
                self.logged_index = None;
                true
            }
            None => false,
        }
    }

    /// Fetches the next page of the current list, if it has one.
    pub fn load_more(&mut self) {
        if self.loading || self.stack.len() != 1 {
            return;
        }
        let Some(offset) = self.stack[0].next_offset else { return };
        self.fetch(Request::More, move |client| async move {
            let albums = client.get_album_list("alphabeticalByName", ALBUM_PAGE_SIZE, offset).await?;
            let next = next_page_offset(albums.len(), offset);
            Ok((albums.into_iter().map(Entry::Album).collect(), next))
        });
    }

    /// Searches the server and opens the results as a new level.
    pub fn search(&mut self, query: &str) {
        let query = query.trim().to_string();
        if self.loading || query.is_empty() {
            return;
        }
        let title = format!("Search: {}", query);
        self.fetch(Request::Open { title, numbered: false }, move |client| async move {
            let results = client.search(&query).await?;
            let entries = results.artists.into_iter().map(Entry::Artist)
                .chain(results.albums.into_iter().map(Entry::Album))
                .chain(results.songs.into_iter().map(Entry::Song))
                .collect();
            Ok((entries, None))
        });
    }

    fn load_root(&mut self, tab: BrowseTab) {
        match tab {
            BrowseTab::Artists => self.fetch(Request::Root(tab), |client| async move {
                let artists = client.get_artists().await?;
                Ok((artists.into_iter().map(Entry::Artist).collect(), None))
            }),
            BrowseTab::Albums => self.fetch(Request::Root(tab), |client| async move {
                let albums = client.get_album_list("alphabeticalByName", ALBUM_PAGE_SIZE, 0).await?;
                let next = next_page_offset(albums.len(), 0);
                Ok((albums.into_iter().map(Entry::Album).collect(), next))
            }),
            BrowseTab::Playlists => self.fetch(Request::Root(tab), |client| async move {
                let playlists = client.get_playlists().await?;
                Ok((playlists.into_iter().map(Entry::Playlist).collect(), None))
            }),
        }
    }

    /// Runs `load` on a background task; poll() applies the result.
    fn fetch<F, Fut>(&mut self, request: Request, load: F)
    where
        F: FnOnce(SubsonicClient) -> Fut,
        Fut: std::future::Future<Output = super::client::Result<(Vec<Entry>, Option<u32>)>> + Send + 'static,
    {
        let Some(client) = self.client.clone() else { return };
        let future = load(client);
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = future.await.map_err(|e| e.to_string());
            let _ = tx.send(Loaded { request, result });
        });
        self.pending = Some(rx);
        self.loading = true;
        self.error = None;
    }

    /// Applies a finished background fetch. Call every tick.
    pub fn poll(&mut self) {
        let Some(rx) = self.pending.as_mut() else { return };
        let loaded = match rx.try_recv() {
            Ok(loaded) => loaded,
            Err(oneshot::error::TryRecvError::Empty) => return,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.pending = None;
                self.loading = false;
                self.error = Some("Request was interrupted".to_string());
                return;
            }
        };
        self.pending = None;
        self.loading = false;

        let (entries, next_offset) = match loaded.result {
            Ok(page) => page,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        match loaded.request {
            Request::Root(tab) => {
                self.stack = vec![Level::new(tab.title().to_string(), entries, next_offset)];
            }
            Request::Open { title, numbered } => {
                let mut level = Level::new(title, entries, next_offset);
                level.numbered = numbered;
                self.stack.push(level);
            }
            Request::More => {
                if let Some(level) = self.stack.last_mut() {
                    level.entries.extend(entries);
                    level.next_offset = next_offset;
                }
            }
        }
    }

    // ── Playback ──────────────────────────────────────────────────────

    /// The queue, if Subsonic music is what the player is playing. Radio
    /// replacing the session (play_url) leaves a stale queue behind; this
    /// ignores it until tick() clears it.
    pub fn active_queue<'a>(&'a self, player: &Player) -> Option<&'a SubsonicQueue> {
        self.queue.as_ref().filter(|_| player.is_playlist())
    }

    pub fn toggle_pause(&self, player: &mut Player) {
        if self.active_queue(player).is_some() {
            player.toggle_pause();
        }
    }

    pub fn next_track(&self, player: &mut Player) {
        if let Some(queue) = self.active_queue(player) {
            queue.next(player);
        }
    }

    pub fn previous_track(&self, player: &mut Player) {
        if let Some(queue) = self.active_queue(player) {
            queue.previous(player);
        }
    }

    pub fn seek(&self, player: &mut Player, seconds: f64) {
        if self.active_queue(player).is_some() {
            player.seek_relative(seconds);
        }
    }

    /// Per-tick housekeeping. Returns a song-log entry when a new track
    /// starts, and whether the queue just played through to the end.
    pub fn tick(&mut self, player: &Player) -> (Option<SongLogEntry>, bool) {
        if self.queue.is_some() && !player.is_playlist() {
            // Radio (or stop) took over the player
            self.queue = None;
            self.logged_index = None;
        }
        let Some(queue) = &self.queue else { return (None, false) };

        if queue.is_finished(player) {
            self.queue = None;
            self.logged_index = None;
            return (None, true);
        }

        let index = queue.current_index(player);
        if index.is_none() || index == self.logged_index {
            return (None, false);
        }
        self.logged_index = index;
        let entry = queue.current(player).map(|song| SongLogEntry {
            title: match &song.artist {
                Some(artist) => format!("{} — {}", artist, song.title),
                None => song.title.clone(),
            },
            station: song.album.clone().unwrap_or_else(|| "Subsonic".to_string()),
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
        });
        (entry, false)
    }
}

/// The songs in `entries` (skipping artists/albums mixed into search
/// results) and the index among them of the entry at `selected`.
fn songs_from(entries: &[Entry], selected: usize) -> Option<(Vec<Song>, usize)> {
    if !matches!(entries.get(selected), Some(Entry::Song(_))) {
        return None;
    }
    let start = entries[..selected].iter().filter(|e| matches!(e, Entry::Song(_))).count();
    let songs = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Song(song) => Some(song.clone()),
            _ => None,
        })
        .collect();
    Some((songs, start))
}

/// A full page means there may be more; a short page is the last.
fn next_page_offset(fetched: usize, offset: u32) -> Option<u32> {
    (fetched as u32 >= ALBUM_PAGE_SIZE).then(|| offset + fetched as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str) -> Entry {
        Entry::Song(Song {
            id: id.into(), title: id.into(), artist: None, album: None, album_id: None,
            track: None, duration: None, bit_rate: None, suffix: None,
        })
    }

    fn artist(id: &str) -> Entry {
        Entry::Artist(Artist { id: id.into(), name: id.into(), album_count: None })
    }

    #[test]
    fn songs_from_skips_non_songs() {
        let entries = vec![artist("a"), song("s1"), artist("b"), song("s2"), song("s3")];
        let (songs, start) = songs_from(&entries, 3).unwrap();
        let ids: Vec<_> = songs.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1", "s2", "s3"]);
        assert_eq!(start, 1);
    }

    #[test]
    fn songs_from_requires_a_selected_song() {
        let entries = vec![artist("a"), song("s1")];
        assert!(songs_from(&entries, 0).is_none());
        assert!(songs_from(&entries, 5).is_none());
    }

    #[test]
    fn paging_stops_on_a_short_page() {
        assert_eq!(next_page_offset(100, 0), Some(100));
        assert_eq!(next_page_offset(100, 100), Some(200));
        assert_eq!(next_page_offset(42, 200), None);
    }

    #[test]
    fn unconfigured_session_reports_why() {
        let session = SubsonicSession::new(&SubsonicConfig::default());
        assert!(session.setup_error.is_some());
        assert!(session.current_level().is_none());
    }
}
