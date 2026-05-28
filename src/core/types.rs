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