use crate::audio::pipe::{self as audio_pipe, SharedAnalysis};
use crate::audio::player::Player;
use crate::audio::visualizer::Visualizer;
use crate::storage::config::{Config, KeyBindings};
use crate::storage::favorites::FavoritesStore;
use crate::storage::history::HistoryStore;

use super::perf::PerfStats;
use super::radio::{self, FetchResult};
use super::types::*;

use tokio::sync::oneshot;

pub struct App {
    pub stations: Vec<radiobrowser::ApiStation>,
    pub selected_index: usize,
    pub player: Player,
    pub volume: u32,
    pub search_query: String,
    pub input_mode: InputMode,
    pub categories: Vec<&'static str>,
    pub category_index: usize,
    pub active_panel: ActivePanel,
    pub overlay: Overlay,
    pub favorites: FavoritesStore,
    pub history: HistoryStore,
    pub fav_selected_index: usize,
    pub hist_selected_index: usize,
    pub visualizer: Visualizer,
    pub now_playing: Option<NowPlaying>,
    pub status_message: Option<String>,
    /// Error message displayed in the Now Playing panel (e.g. mpv not found)
    pub error_message: Option<String>,
    pub page_size: u32,
    pub has_more: bool,
    pub last_query: QueryKind,
    pub analysis: SharedAnalysis,
    /// When the app was started (for session time)
    pub session_start: std::time::Instant,
    /// Rolling log of song titles seen this session (newest first)
    pub song_log: Vec<SongLogEntry>,
    /// Track the last media title to detect changes
    pub last_media_title: Option<String>,
    /// Performance profiler (toggle with ` key)
    pub perf: PerfStats,
    pub show_perf: bool,
    /// Current tick rate in ms (adjustable with < > keys when perf overlay is shown)
    pub tick_rate_ms: u64,
    /// FFT updates per second (measured from the reader thread's fft_count)
    pub fft_rate: f64,
    /// Previous fft_count snapshot for computing rate
    fft_count_prev: u64,
    /// Timestamp of last FFT rate measurement
    fft_rate_time: std::time::Instant,
    /// Country code for blended local/global station discovery (from config)
    pub country_code: String,
    /// Remappable keybindings (persisted to config)
    pub keybindings: KeyBindings,
    /// Currently selected action index in settings overlay
    pub settings_selected: usize,
    /// When true, the settings overlay is waiting for a keypress to rebind
    /// The tuple is (action_index, is_alt_slot)
    pub settings_awaiting_key: Option<(usize, bool)>,
    /// True while a background RadioBrowser fetch is in flight
    pub is_loading: bool,
    /// Receiver for the result of a background fetch (checked each tick)
    pub pending_fetch: Option<oneshot::Receiver<FetchResult>>,
    /// Currently selected index in the genre picker overlay
    pub genre_selected: usize,
    /// Active color theme for the player UI
    pub theme: crate::ui::themes::Theme,
    /// Currently selected index in the theme picker overlay
    pub theme_selected: usize,
    /// Whether the visualizer is enabled (can be toggled at runtime)
    pub visualizer_enabled: bool,
    /// When true, theme backgrounds are cleared so the terminal's own
    /// (possibly transparent) background shows through instead
    pub transparent_bg: bool,
    /// Cached " │ X search │ Y genre │ ..." header hint string, rebuilt
    /// only when keybindings actually change (via save_config()) rather
    /// than reformatted from scratch every single frame.
    pub header_hint: String,
    /// Cached " │ vis: off (X)" suffix, empty when the visualizer is on.
    /// Rebuilt alongside header_hint for the same reason.
    pub header_vis_status: String,
}

impl App {
    pub fn new(stations: Vec<radiobrowser::ApiStation>) -> Self {
        let has_more = stations.len() as u32 >= 30;
        let analysis = audio_pipe::new_shared_analysis();
        let config = Config::load();
        let mut player = Player::new(analysis.clone());
        player.visualizer_enabled = config.visualizer_enabled;

        let mut app = Self {
            stations,
            selected_index: 0,
            player,
            volume: config.volume,
            search_query: String::new(),
            input_mode: InputMode::Normal,
            categories: vec![
                "Lo-fi", "Jazz", "Rock", "Classical", "Chill",
                "Blues", "Electronic", "Ambient", "Pop", "Metal",
                "Hip Hop", "R&B", "Soul", "Funk", "Reggae",
                "Country", "Folk", "Punk", "Indie", "Latin",
                "House", "Techno", "Drum and Bass", "Trance", "Dubstep",
                "Synthwave", "Retrowave", "Vaporwave",
                "Soundtrack", "World", "Disco", "Ska",
                "News", "Talk", "Chat", "Sports",
            ],
            category_index: 0,
            active_panel: ActivePanel::from_config_str(&config.default_panel),
            overlay: Overlay::None,
            favorites: FavoritesStore::load(),
            history: HistoryStore::load(),
            fav_selected_index: 0,
            hist_selected_index: 0,
            visualizer: Visualizer::new(),
            now_playing: None,
            status_message: None,
            error_message: None,
            page_size: 30,
            has_more,
            last_query: QueryKind::Tag("lo-fi".to_string()),
            analysis,
            session_start: std::time::Instant::now(),
            song_log: Vec::new(),
            last_media_title: None,
            perf: PerfStats::new(),
            show_perf: false,
            tick_rate_ms: config.tick_rate_ms,
            fft_rate: 0.0,
            fft_count_prev: 0,
            fft_rate_time: std::time::Instant::now(),
            country_code: config.country_code.clone(),
            keybindings: config.keybindings,
            settings_selected: 0,
            settings_awaiting_key: None,
            is_loading: false,
            pending_fetch: None,
            genre_selected: 0,
            theme: crate::ui::themes::Theme::by_name(&config.theme).with_transparency(config.transparent_bg),
            theme_selected: {
                let all = crate::ui::themes::Theme::all();
                all.iter().position(|t| t.name.eq_ignore_ascii_case(&config.theme)).unwrap_or(0)
            },
            visualizer_enabled: config.visualizer_enabled,
            transparent_bg: config.transparent_bg,
            header_hint: String::new(),
            header_vis_status: String::new(),
        };
        app.rebuild_header_hint();
        app
    }

    /// Rebuild the cached header hint strings from current keybindings and
    /// visualizer state. Every place that mutates keybindings or toggles
    /// the visualizer already calls save_config() right afterward, so
    /// hooking the rebuild there (rather than at each mutation site
    /// individually) keeps this in sync without needing to remember to
    /// invalidate it in five different places.
    fn rebuild_header_hint(&mut self) {
        use crate::storage::config::keycode_to_string;

        let search_key = keycode_to_string(self.keybindings.search.primary);
        let genre_key = keycode_to_string(self.keybindings.genre_picker.primary);
        let theme_key = keycode_to_string(self.keybindings.theme_picker.primary);
        let help_key = keycode_to_string(self.keybindings.help.primary);
        let settings_key = keycode_to_string(self.keybindings.settings.primary);
        let vis_key = keycode_to_string(self.keybindings.visualizer_toggle.primary);

        self.header_hint = format!(
            "  │  {} search  │  {} genre  │  {} theme  │  {} help  │  {} settings  │  {} vizualizer toggle",
            search_key, genre_key, theme_key, help_key, settings_key, vis_key
        );

        self.header_vis_status = if self.visualizer_enabled {
            String::new()
        } else {
            format!("  │  vis: off ({})", vis_key)
        };
    }

    pub fn next(&mut self) {
        match self.active_panel {
            ActivePanel::Stations => {
                if !self.stations.is_empty() && self.selected_index < self.stations.len() - 1 {
                    self.selected_index += 1;
                }
            }
            ActivePanel::Favorites => {
                let len = self.favorites.entries.len();
                if len > 0 && self.fav_selected_index < len - 1 {
                    self.fav_selected_index += 1;
                }
            }
            ActivePanel::History => {
                let len = self.history.entries.len();
                if len > 0 && self.hist_selected_index < len - 1 {
                    self.hist_selected_index += 1;
                }
            }
        }
    }

    pub fn previous(&mut self) {
        match self.active_panel {
            ActivePanel::Stations => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            ActivePanel::Favorites => {
                if self.fav_selected_index > 0 {
                    self.fav_selected_index -= 1;
                }
            }
            ActivePanel::History => {
                if self.hist_selected_index > 0 {
                    self.hist_selected_index -= 1;
                }
            }
        }
    }

    pub fn switch_category(&mut self) {
        if self.is_loading { return; }
        self.category_index = (self.category_index + 1) % self.categories.len();
        let genre = self.categories[self.category_index];
        let tag = genre.to_lowercase();
        self.start_tag_fetch(tag, genre.to_string());
    }

    pub fn switch_category_back(&mut self) {
        if self.is_loading { return; }
        if self.category_index == 0 {
            self.category_index = self.categories.len() - 1;
        } else {
            self.category_index -= 1;
        }
        let genre = self.categories[self.category_index];
        let tag = genre.to_lowercase();
        self.start_tag_fetch(tag, genre.to_string());
    }

    /// Spawn a background fetch for stations by tag and stash the receiver.
    fn start_tag_fetch(&mut self, tag: String, genre: String) {
        self.is_loading = true;
        self.status_message = Some(format!("Loading '{}'...", genre));
        let country_code = self.country_code.clone();
        let query_tag = tag.clone();

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = radio::fetch_blended_by_tag(tag, country_code).await;
            let fetch_result = match result {
                Ok(stations) => {
                    let msg = format!("Loaded {} stations for '{}'", stations.len(), genre);
                    FetchResult::Replace {
                        stations,
                        query: QueryKind::Tag(query_tag),
                        message: msg,
                        switch_to_stations: true,
                    }
                }
                Err(e) => FetchResult::Error(format!("⚠ Failed to load '{}': {}", genre, e)),
            };
            let _ = tx.send(fetch_result);
        });
        self.pending_fetch = Some(rx);
    }

    /// Select a genre by index from the genre picker overlay.
    pub fn select_genre(&mut self, index: usize) {
        if index >= self.categories.len() || self.is_loading { return; }
        self.category_index = index;
        let genre = self.categories[self.category_index];
        let tag = genre.to_lowercase();
        self.start_tag_fetch(tag, genre.to_string());
    }

    pub fn play(&mut self) {
        // Clear any previous error
        self.error_message = None;

        match self.active_panel {
            ActivePanel::Stations => {
                if let Some(station) = self.stations.get(self.selected_index) {
                    if self.player.play_url(&station.url, self.volume) {
                        self.now_playing = Some(NowPlaying::from_station(station));
                        self.history.add(&station.name, &station.url, &station.tags, &station.country, station.bitrate);
                        self.status_message = Some(format!("♪ Playing: {}", station.name));
                    } else {
                        self.error_message = Some(Self::mpv_error_message());
                    }
                }
            }
            ActivePanel::Favorites => {
                if let Some(fav) = self.favorites.entries.get(self.fav_selected_index) {
                    if self.player.play_url(&fav.url, self.volume) {
                        self.now_playing = Some(NowPlaying {
                            name: fav.name.clone(),
                            genre: fav.genre.clone(),
                            bitrate: fav.bitrate,
                            codec: String::new(),
                            country: fav.country.clone(),
                            url: fav.url.clone(),
                            homepage: String::new(),
                            votes: 0,
                        });
                        self.history.add(&fav.name, &fav.url, &fav.genre, &fav.country, fav.bitrate);
                        self.status_message = Some(format!("♪ Playing: {}", fav.name));
                    } else {
                        self.error_message = Some(Self::mpv_error_message());
                    }
                }
            }
            ActivePanel::History => {
                if let Some(entry) = self.history.entries.get(self.hist_selected_index) {
                    if self.player.play_url(&entry.url, self.volume) {
                        self.now_playing = Some(NowPlaying {
                            name: entry.name.clone(),
                            genre: entry.genre.clone(),
                            bitrate: entry.bitrate,
                            codec: String::new(),
                            country: entry.country.clone(),
                            url: entry.url.clone(),
                            homepage: String::new(),
                            votes: 0,
                        });
                        self.status_message = Some(format!("♪ Playing: {}", entry.name));
                    } else {
                        self.error_message = Some(Self::mpv_error_message());
                    }
                }
            }
        }
    }

    /// Platform-appropriate error message when mpv is not found
    fn mpv_error_message() -> String {
        if cfg!(target_os = "macos") {
            "mpv not found — install with: brew install mpv".to_string()
        } else if cfg!(target_os = "windows") {
            "mpv not found — download from mpv.io or reinstall AetherTune".to_string()
        } else {
            "mpv not found — install with your package manager (e.g. sudo apt install mpv)".to_string()
        }
    }

    pub fn stop(&mut self) {
        self.player.stop();
        self.now_playing = None;
        self.error_message = None;
        self.visualizer.reset();
        self.status_message = Some("Playback stopped".to_string());
    }

    pub fn set_volume(&mut self, delta: i32) {
        let new_vol = self.volume as i32 + delta;
        self.volume = new_vol.clamp(0, 100) as u32;
        self.player.set_volume(self.volume);
        self.status_message = Some(format!("Volume: {}%", self.volume));
        self.save_config();
    }

    /// Persist current tick rate, volume, and keybindings to config file
    pub fn save_config(&mut self) {
        let mut config = Config::load();
        // Only save tick_rate_ms when visualizer is enabled — otherwise we'd
        // overwrite the user's preference with the low-power 200ms value
        if self.visualizer_enabled {
            config.tick_rate_ms = self.tick_rate_ms;
        }
        config.volume = self.volume;
        config.keybindings = self.keybindings.clone();
        config.theme = self.theme.name.to_string();
        config.visualizer_enabled = self.visualizer_enabled;
        config.transparent_bg = self.transparent_bg;
        config.save();

        // Keybindings and visualizer_enabled are the two inputs to the
        // header hint, and every call site that mutates either of them
        // calls save_config() right afterward — so rebuilding here keeps
        // the cache correct without a separate invalidation call at each
        // of those mutation sites.
        self.rebuild_header_hint();
    }

    pub fn toggle_favorite(&mut self) {
        if self.active_panel == ActivePanel::Stations {
            if let Some(station) = self.stations.get(self.selected_index) {
                let was_added = self.favorites.toggle(
                    &station.name,
                    &station.url,
                    &station.tags,
                    &station.country,
                    station.bitrate,
                );
                if was_added {
                    self.status_message = Some(format!("★ Added '{}' to favorites", station.name));
                } else {
                    self.status_message = Some(format!("Removed '{}' from favorites", station.name));
                }
            }
        } else if self.active_panel == ActivePanel::Favorites {
            if let Some(fav) = self.favorites.entries.get(self.fav_selected_index).cloned() {
                self.favorites.toggle(&fav.name, &fav.url, &fav.genre, &fav.country, fav.bitrate);
                if self.fav_selected_index > 0 {
                    self.fav_selected_index -= 1;
                }
                self.status_message = Some(format!("Removed '{}' from favorites", fav.name));
            }
        } else if self.active_panel == ActivePanel::History {
            if let Some(entry) = self.history.entries.get(self.hist_selected_index).cloned() {
                let was_added = self.favorites.toggle(
                    &entry.name,
                    &entry.url,
                    &entry.genre,
                    &entry.country,
                    entry.bitrate,
                );
                if was_added {
                    self.status_message = Some(format!("★ Added '{}' to favorites", entry.name));
                } else {
                    self.status_message = Some(format!("Removed '{}' from favorites", entry.name));
                }
            }
        }
    }

    pub fn is_favorite(&self, url: &str) -> bool {
        self.favorites.entries.iter().any(|f| f.url == url)
    }

    pub fn perform_search(&mut self) {
        if self.is_loading { return; }
        let query = self.search_query.clone();
        if query.is_empty() { return; }

        self.is_loading = true;
        self.status_message = Some(format!("Searching '{}'...", query));
        let country_code = self.country_code.clone();
        let search_query = query.clone();

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = radio::fetch_blended_by_name(query, country_code).await;
            let fetch_result = match result {
                Ok(stations) => {
                    let msg = format!("Found {} stations for '{}'", stations.len(), search_query);
                    FetchResult::Replace {
                        stations,
                        query: QueryKind::Search(search_query),
                        message: msg,
                        switch_to_stations: true,
                    }
                }
                Err(e) => FetchResult::Error(format!("⚠ Search failed: {}", e)),
            };
            let _ = tx.send(fetch_result);
        });
        self.pending_fetch = Some(rx);
    }

    pub fn load_more(&mut self) {
        if self.is_loading { return; }
        if !self.has_more {
            self.status_message = Some("No more stations to load".to_string());
            return;
        }

        self.is_loading = true;
        self.status_message = Some("Loading more stations...".to_string());
        let offset = self.stations.len().to_string();
        let limit = self.page_size.to_string();
        let query = self.last_query.clone();

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = radio::fetch_more(query, offset, limit).await;
            let fetch_result = match result {
                Ok(stations) => FetchResult::Append { stations },
                Err(e) => FetchResult::Error(format!("⚠ Failed to load more: {}", e)),
            };
            let _ = tx.send(fetch_result);
        });
        self.pending_fetch = Some(rx);
    }

    pub fn cycle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Stations => ActivePanel::Favorites,
            ActivePanel::Favorites => ActivePanel::History,
            ActivePanel::History => ActivePanel::Stations,
        };
    }

    /// Check if a background fetch has completed and apply the result.
    /// Call this every tick from the main loop.
    pub fn poll_fetch(&mut self) {
        let rx = match self.pending_fetch.as_mut() {
            Some(rx) => rx,
            None => return,
        };

        // Non-blocking check — try_recv returns Ok if ready, Err if not yet
        match rx.try_recv() {
            Ok(result) => {
                self.pending_fetch = None;
                self.is_loading = false;
                match result {
                    FetchResult::Replace { stations, query, message, switch_to_stations } => {
                        self.has_more = stations.len() as u32 >= self.page_size;
                        self.stations = stations;
                        self.last_query = query;
                        self.selected_index = 0;
                        if switch_to_stations {
                            self.active_panel = ActivePanel::Stations;
                            self.status_message = Some(message);
                        }
                        // Silent startup preload: stations are ready for whenever the
                        // user tabs over, but we don't steal their panel or narrate it.
                    }
                    FetchResult::Append { mut stations } => {
                        let fetched = stations.len();
                        self.has_more = fetched as u32 >= self.page_size;

                        // Filter out duplicates by URL
                        let existing_urls: std::collections::HashSet<String> =
                            self.stations.iter().map(|s| s.url.clone()).collect();
                        let mut added = 0;
                        for station in stations.drain(..) {
                            if !existing_urls.contains(&station.url) {
                                self.stations.push(station);
                                added += 1;
                            }
                        }
                        self.status_message = Some(format!(
                            "Loaded {} more stations (total: {})",
                            added,
                            self.stations.len()
                        ));
                    }
                    FetchResult::Error(msg) => {
                        self.status_message = Some(msg);
                    }
                }
            }
            Err(oneshot::error::TryRecvError::Empty) => {
                // Still in flight — do nothing
            }
            Err(oneshot::error::TryRecvError::Closed) => {
                // Sender dropped (task panicked?) — clean up
                self.pending_fetch = None;
                self.is_loading = false;
                self.status_message = Some("⚠ Station fetch was interrupted".to_string());
            }
        }
    }

    /// Kick off the initial station fetch (called from main after App is constructed).
    /// The app starts immediately with an empty station list; stations arrive via poll_fetch.
    pub fn start_initial_fetch(&mut self) {
        self.is_loading = true;
        self.status_message = Some("Connecting to RadioBrowser...".to_string());
        let country_code = self.country_code.clone();

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let result = radio::fetch_blended_by_tag("lo-fi".to_string(), country_code).await;
            let fetch_result = match result {
                Ok(stations) => {
                    let msg = format!("Loaded {} stations", stations.len());
                    FetchResult::Replace {
                        stations,
                        query: QueryKind::Tag("lo-fi".to_string()),
                        message: msg,
                        switch_to_stations: false,
                    }
                }
                Err(e) => FetchResult::Error(
                    format!("⚠ Could not load stations: {}. Try searching or switching genres.", e)
                ),
            };
            let _ = tx.send(fetch_result);
        });
        self.pending_fetch = Some(rx);
    }

    /// Check if the media title changed and log it.
    /// Call this each tick after player.poll().
    pub fn check_song_change(&mut self) {
        let current_title = self.player.media_title.clone();

        if current_title != self.last_media_title {
            if let Some(ref title) = current_title {
                if !title.is_empty() && !Self::is_stream_noise(title, &self.now_playing) {
                    let station_name = self
                        .now_playing
                        .as_ref()
                        .map(|np| np.name.clone())
                        .unwrap_or_default();

                    let timestamp = chrono::Local::now().format("%H:%M").to_string();

                    self.song_log.insert(
                        0,
                        SongLogEntry {
                            title: title.clone(),
                            station: station_name,
                            timestamp,
                        },
                    );

                    // Keep max 50 entries
                    if self.song_log.len() > 50 {
                        self.song_log.truncate(50);
                    }
                }
            }
            self.last_media_title = current_title;
        }
    }

    /// Update the FFT updates/sec measurement.
    /// Called each tick; computes rate over the elapsed interval.
    pub fn update_fft_rate(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.fft_rate_time).as_secs_f64();

        // Update every ~1 second to avoid jittery measurements
        if elapsed >= 1.0 {
            let count = self.analysis.read().fft_count;
            let delta = count.saturating_sub(self.fft_count_prev);
            self.fft_rate = delta as f64 / elapsed;
            self.fft_count_prev = count;
            self.fft_rate_time = now;
        }
    }

    /// Returns true if a media title is just stream noise rather than a real song.
    /// Filters out raw URLs, internal stream names, and other non-song metadata
    /// that mpv reports before ICY metadata arrives.
    fn is_stream_noise(title: &str, now_playing: &Option<NowPlaying>) -> bool {
        let t = title.trim();

        // Raw URLs
        if t.starts_with("http://") || t.starts_with("https://") {
            return true;
        }

        // Matches the station's stream URL
        if let Some(np) = now_playing {
            if t == np.url || np.url.contains(t) {
                return true;
            }
        }

        // Internal stream names: no spaces and looks like a slug/filename
        // Real song titles virtually always have spaces (e.g. "Artist - Song")
        if !t.contains(' ') && (t.contains('_') || t.contains('.') || t.ends_with("mp3")) {
            return true;
        }

        false
    }

    /// Format session duration as "Xh Ym" or "Ym Zs"
    pub fn session_duration_str(&self) -> String {
        let elapsed = self.session_start.elapsed().as_secs();
        let hours = elapsed / 3600;
        let minutes = (elapsed % 3600) / 60;
        let seconds = elapsed % 60;

        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}