use crate::audio::pipe::{self as audio_pipe, SharedAnalysis};
use crate::audio::player::Player;
use crate::audio::visualizer::Visualizer;
use crate::storage::favorites::FavoritesStore;
use crate::storage::history::HistoryStore;

#[derive(PartialEq, Clone)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq, Clone)]
pub enum ActivePanel {
    Stations,
    Favorites,
    History,
}

#[derive(PartialEq, Clone)]
pub enum Overlay {
    None,
    Help,
    StationDetail,
}

/// Lightweight per-frame performance counters.
/// Tracks timing for each phase of the main loop so we can
/// see the cost of increasing the tick rate.
pub struct PerfStats {
    /// Ring buffer of recent frame timings
    samples: Vec<FrameTiming>,
    write_idx: usize,
    capacity: usize,
}

#[derive(Clone, Copy, Default)]
pub struct FrameTiming {
    pub draw_us: u64,
    /// Idle time spent in event::poll() waiting for input or timeout
    pub event_wait_us: u64,
    /// Actual work done handling key events after poll returns
    pub event_handle_us: u64,
    pub poll_us: u64,
    pub vis_us: u64,
    pub total_us: u64,
}

impl FrameTiming {
    /// CPU work only — excludes the idle poll wait
    pub fn work_us(&self) -> u64 {
        self.draw_us + self.event_handle_us + self.poll_us + self.vis_us
    }
}

impl PerfStats {
    pub fn new() -> Self {
        let capacity = 120; // ~10 seconds at 12 FPS, more at higher rates
        Self {
            samples: vec![FrameTiming::default(); capacity],
            write_idx: 0,
            capacity,
        }
    }

    pub fn record(&mut self, timing: FrameTiming) {
        self.samples[self.write_idx] = timing;
        self.write_idx = (self.write_idx + 1) % self.capacity;
    }

    /// Returns averages over recent samples
    pub fn summary(&self) -> FrameTiming {
        let mut avg = FrameTiming::default();
        let mut count = 0u64;
        for s in &self.samples {
            if s.total_us > 0 {
                avg.draw_us += s.draw_us;
                avg.event_wait_us += s.event_wait_us;
                avg.event_handle_us += s.event_handle_us;
                avg.poll_us += s.poll_us;
                avg.vis_us += s.vis_us;
                avg.total_us += s.total_us;
                count += 1;
            }
        }
        if count > 0 {
            avg.draw_us /= count;
            avg.event_wait_us /= count;
            avg.event_handle_us /= count;
            avg.poll_us /= count;
            avg.vis_us /= count;
            avg.total_us /= count;
        }
        avg
    }

    pub fn max(&self) -> FrameTiming {
        let mut m = FrameTiming::default();
        for s in &self.samples {
            m.draw_us = m.draw_us.max(s.draw_us);
            m.event_wait_us = m.event_wait_us.max(s.event_wait_us);
            m.event_handle_us = m.event_handle_us.max(s.event_handle_us);
            m.poll_us = m.poll_us.max(s.poll_us);
            m.vis_us = m.vis_us.max(s.vis_us);
            m.total_us = m.total_us.max(s.total_us);
        }
        m
    }
}

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
}

#[derive(Clone)]
pub struct SongLogEntry {
    pub title: String,
    pub station: String,
    pub timestamp: String,
}

#[derive(Clone)]
pub enum QueryKind {
    Tag(String),
    Search(String),
}

#[derive(Clone)]
pub struct NowPlaying {
    pub name: String,
    pub genre: String,
    pub bitrate: u32,
    pub codec: String,
    pub country: String,
    pub url: String,
    pub homepage: String,
    pub votes: i32,
}

impl NowPlaying {
    pub fn from_station(station: &radiobrowser::ApiStation) -> Self {
        Self {
            name: station.name.clone(),
            genre: station.tags.clone(),
            bitrate: station.bitrate,
            codec: station.codec.clone(),
            country: station.country.clone(),
            url: station.url.clone(),
            homepage: station.homepage.clone(),
            votes: station.votes,
        }
    }
}

impl App {
    pub fn new(stations: Vec<radiobrowser::ApiStation>) -> Self {
        let has_more = stations.len() as u32 >= 30;
        let analysis = audio_pipe::new_shared_analysis();
        Self {
            stations,
            selected_index: 0,
            player: Player::new(analysis.clone()),
            volume: 50,
            search_query: String::new(),
            input_mode: InputMode::Normal,
            categories: vec![
                "Lo-fi", "Jazz", "Rock", "Classical", "Chill",
                "Blues", "Electronic", "Ambient", "Pop", "Metal",
            ],
            category_index: 0,
            active_panel: ActivePanel::Stations,
            overlay: Overlay::None,
            favorites: FavoritesStore::load(),
            history: HistoryStore::load(),
            fav_selected_index: 0,
            hist_selected_index: 0,
            visualizer: Visualizer::new(),
            now_playing: None,
            status_message: None,
            page_size: 30,
            has_more,
            last_query: QueryKind::Tag("lo-fi".to_string()),
            analysis,
            session_start: std::time::Instant::now(),
            song_log: Vec::new(),
            last_media_title: None,
            perf: PerfStats::new(),
            show_perf: false,
            tick_rate_ms: 30,
        }
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

    pub async fn switch_category(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.category_index = (self.category_index + 1) % self.categories.len();
        let genre = self.categories[self.category_index];
        let tag = genre.to_lowercase();

        let client = radiobrowser::RadioBrowserAPI::new().await?;
        self.stations = client.get_stations().tag(tag.clone()).limit("30").send().await?;
        self.has_more = self.stations.len() as u32 >= self.page_size;
        self.last_query = QueryKind::Tag(tag);
        self.selected_index = 0;
        self.status_message = Some(format!("Loaded {} stations for '{}'", self.stations.len(), genre));
        Ok(())
    }

    pub async fn switch_category_back(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.category_index == 0 {
            self.category_index = self.categories.len() - 1;
        } else {
            self.category_index -= 1;
        }
        let genre = self.categories[self.category_index];
        let tag = genre.to_lowercase();

        let client = radiobrowser::RadioBrowserAPI::new().await?;
        self.stations = client.get_stations().tag(tag.clone()).limit("30").send().await?;
        self.has_more = self.stations.len() as u32 >= self.page_size;
        self.last_query = QueryKind::Tag(tag);
        self.selected_index = 0;
        self.status_message = Some(format!("Loaded {} stations for '{}'", self.stations.len(), genre));
        Ok(())
    }

    pub fn play(&mut self) {
        match self.active_panel {
            ActivePanel::Stations => {
                if let Some(station) = self.stations.get(self.selected_index) {
                    self.player.play_url(&station.url, self.volume);
                    self.now_playing = Some(NowPlaying::from_station(station));
                    self.history.add(&station.name, &station.url, &station.tags, &station.country, station.bitrate);
                    self.status_message = Some(format!("♪ Playing: {}", station.name));
                }
            }
            ActivePanel::Favorites => {
                if let Some(fav) = self.favorites.entries.get(self.fav_selected_index) {
                    self.player.play_url(&fav.url, self.volume);
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
                }
            }
            ActivePanel::History => {
                if let Some(entry) = self.history.entries.get(self.hist_selected_index) {
                    self.player.play_url(&entry.url, self.volume);
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
                }
            }
        }
    }

    pub fn stop(&mut self) {
        self.player.stop();
        self.now_playing = None;
        self.visualizer.reset();
        self.status_message = Some("Playback stopped".to_string());
    }

    pub fn set_volume(&mut self, delta: i32) {
        let new_vol = self.volume as i32 + delta;
        self.volume = new_vol.clamp(0, 100) as u32;
        self.player.set_volume(self.volume);
        self.status_message = Some(format!("Volume: {}%", self.volume));
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
        }
    }

    pub fn is_favorite(&self, url: &str) -> bool {
        self.favorites.entries.iter().any(|f| f.url == url)
    }

    pub async fn perform_search(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let client = radiobrowser::RadioBrowserAPI::new().await?;
        let stations = client
            .get_stations()
            .name(&self.search_query)
            .limit("30")
            .send()
            .await?;

        let count = stations.len();
        self.has_more = count as u32 >= self.page_size;
        self.last_query = QueryKind::Search(self.search_query.clone());
        self.stations = stations;
        self.selected_index = 0;
        self.active_panel = ActivePanel::Stations;
        self.status_message = Some(format!("Found {} stations for '{}'", count, self.search_query));
        Ok(())
    }

    pub async fn load_more(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.has_more {
            self.status_message = Some("No more stations to load".to_string());
            return Ok(());
        }

        let offset = self.stations.len().to_string();
        let limit = self.page_size.to_string();
        let client = radiobrowser::RadioBrowserAPI::new().await?;

        let new_stations = match &self.last_query {
            QueryKind::Tag(tag) => {
                client.get_stations().tag(tag).offset(offset).limit(limit).send().await?
            }
            QueryKind::Search(query) => {
                client.get_stations().name(query).offset(offset).limit(limit).send().await?
            }
        };

        let fetched = new_stations.len();
        self.has_more = fetched as u32 >= self.page_size;

        // Filter out duplicates by URL
        let existing_urls: std::collections::HashSet<String> =
            self.stations.iter().map(|s| s.url.clone()).collect();
        let mut added = 0;
        for station in new_stations {
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
        Ok(())
    }

    pub fn cycle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Stations => ActivePanel::Favorites,
            ActivePanel::Favorites => ActivePanel::History,
            ActivePanel::History => ActivePanel::Stations,
        };
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

                    // Get timestamp via date command
                    let timestamp = std::process::Command::new("date")
                        .arg("+%H:%M")
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

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