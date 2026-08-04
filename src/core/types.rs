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

impl ActivePanel {
    /// Cycle order used both by the in-app Tab key and the boot-menu default-panel picker.
    pub const ALL: [ActivePanel; 3] = [ActivePanel::Stations, ActivePanel::Favorites, ActivePanel::History];

    pub fn as_str(&self) -> &'static str {
        match self {
            ActivePanel::Stations => "Stations",
            ActivePanel::Favorites => "Favorites",
            ActivePanel::History => "History",
        }
    }

    /// Parses a config value, falling back to Stations for anything unrecognized
    /// (covers missing config, first run, or a hand-edited/corrupted config.json).
    pub fn from_config_str(s: &str) -> ActivePanel {
        match s {
            "Favorites" => ActivePanel::Favorites,
            "History" => ActivePanel::History,
            _ => ActivePanel::Stations,
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum Overlay {
    None,
    Help,
    StationDetail,
    Settings,
    GenrePicker,
    ThemePicker,
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

#[derive(Clone)]
pub struct SongLogEntry {
    pub title: String,
    pub station: String,
    pub timestamp: String,
}