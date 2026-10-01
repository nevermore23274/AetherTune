use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use crossterm::event::KeyCode;
use crate::subsonic::config::SubsonicConfig;

const DEFAULT_TICK_RATE_MS: u64 = 30;
const DEFAULT_VOLUME: u32 = 50;

/// Every remappable action in the app.
/// Each action can have one or two key bindings (primary + optional alternate).
#[derive(Clone)]
pub struct KeyBinding {
    pub primary: KeyCode,
    pub alt: Option<KeyCode>,
}

impl KeyBinding {
    pub fn new(primary: KeyCode) -> Self {
        Self { primary, alt: None }
    }

    pub fn with_alt(primary: KeyCode, alt: KeyCode) -> Self {
        Self { primary, alt: Some(alt) }
    }

    /// Returns true if the given KeyCode matches either binding
    pub fn matches(&self, code: KeyCode) -> bool {
        self.primary == code || self.alt.map_or(false, |a| a == code)
    }
}

/// All remappable actions. Names here become JSON keys under "keybindings".
#[derive(Clone)]
pub struct KeyBindings {
    pub navigate_down: KeyBinding,
    pub navigate_up: KeyBinding,
    pub play: KeyBinding,
    pub stop: KeyBinding,
    pub volume_up: KeyBinding,
    pub volume_down: KeyBinding,
    pub search: KeyBinding,
    pub toggle_favorite: KeyBinding,
    pub station_detail: KeyBinding,
    pub load_more: KeyBinding,
    pub cycle_panel: KeyBinding,
    pub genre_next: KeyBinding,
    pub genre_prev: KeyBinding,
    pub genre_picker: KeyBinding,
    pub theme_picker: KeyBinding,
    pub visualizer_toggle: KeyBinding,
    pub help: KeyBinding,
    pub perf_toggle: KeyBinding,
    pub perf_tick_slower: KeyBinding,
    pub perf_tick_faster: KeyBinding,
    pub settings: KeyBinding,
    pub quit: KeyBinding,
    // Subsonic (music) actions
    pub toggle_source: KeyBinding,
    pub back: KeyBinding,
    pub pause: KeyBinding,
    pub next_track: KeyBinding,
    pub prev_track: KeyBinding,
    pub seek_forward: KeyBinding,
    pub seek_back: KeyBinding,
}

impl KeyBindings {
    /// All actions as (json_key, display_label, binding_ref) for iteration
    pub fn all_actions(&self) -> Vec<(&'static str, &'static str, &KeyBinding)> {
        vec![
            ("navigate_down",    "Navigate Down",       &self.navigate_down),
            ("navigate_up",      "Navigate Up",         &self.navigate_up),
            ("play",             "Play Station",        &self.play),
            ("stop",             "Stop Playback",       &self.stop),
            ("volume_up",        "Volume Up",           &self.volume_up),
            ("volume_down",      "Volume Down",         &self.volume_down),
            ("search",           "Search Stations",     &self.search),
            ("toggle_favorite",  "Toggle Favorite",     &self.toggle_favorite),
            ("station_detail",   "Station Details",     &self.station_detail),
            ("load_more",        "Load More Stations",  &self.load_more),
            ("cycle_panel",      "Cycle Panel",         &self.cycle_panel),
            ("genre_next",       "Next Genre",          &self.genre_next),
            ("genre_prev",       "Previous Genre",      &self.genre_prev),
            ("genre_picker",     "Genre Picker",        &self.genre_picker),
            ("theme_picker",     "Theme Picker",        &self.theme_picker),
            ("visualizer_toggle","Toggle Visualizer",   &self.visualizer_toggle),
            ("help",             "Help Overlay",        &self.help),
            ("perf_toggle",      "Perf Profiler",       &self.perf_toggle),
            ("perf_tick_slower",  "Tick Rate Slower",   &self.perf_tick_slower),
            ("perf_tick_faster",  "Tick Rate Faster",   &self.perf_tick_faster),
            ("settings",         "Settings",            &self.settings),
            ("quit",             "Quit",                &self.quit),
            ("toggle_source",    "Radio / Subsonic",    &self.toggle_source),
            ("back",             "Back (Subsonic)",     &self.back),
            ("pause",            "Pause / Resume",      &self.pause),
            ("next_track",       "Next Track",          &self.next_track),
            ("prev_track",       "Previous Track",      &self.prev_track),
            ("seek_forward",     "Seek Forward",        &self.seek_forward),
            ("seek_back",        "Seek Back",           &self.seek_back),
        ]
    }

    /// Mutable version for rebinding
    pub fn set_binding(&mut self, json_key: &str, primary: KeyCode, alt: Option<KeyCode>) {
        let binding = match json_key {
            "navigate_down"    => &mut self.navigate_down,
            "navigate_up"      => &mut self.navigate_up,
            "play"             => &mut self.play,
            "stop"             => &mut self.stop,
            "volume_up"        => &mut self.volume_up,
            "volume_down"      => &mut self.volume_down,
            "search"           => &mut self.search,
            "toggle_favorite"  => &mut self.toggle_favorite,
            "station_detail"   => &mut self.station_detail,
            "load_more"        => &mut self.load_more,
            "cycle_panel"      => &mut self.cycle_panel,
            "genre_next"       => &mut self.genre_next,
            "genre_prev"       => &mut self.genre_prev,
            "genre_picker"     => &mut self.genre_picker,
            "theme_picker"     => &mut self.theme_picker,
            "visualizer_toggle" => &mut self.visualizer_toggle,
            "help"             => &mut self.help,
            "perf_toggle"      => &mut self.perf_toggle,
            "perf_tick_slower"  => &mut self.perf_tick_slower,
            "perf_tick_faster"  => &mut self.perf_tick_faster,
            "settings"         => &mut self.settings,
            "quit"             => &mut self.quit,
            "toggle_source"    => &mut self.toggle_source,
            "back"             => &mut self.back,
            "pause"            => &mut self.pause,
            "next_track"       => &mut self.next_track,
            "prev_track"       => &mut self.prev_track,
            "seek_forward"     => &mut self.seek_forward,
            "seek_back"        => &mut self.seek_back,
            _ => return,
        };
        binding.primary = primary;
        binding.alt = alt;
    }

    /// Get the json_key at a given index in all_actions()
    pub fn key_at_index(&self, index: usize) -> Option<&'static str> {
        self.all_actions().get(index).map(|(k, _, _)| *k)
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            navigate_down:    KeyBinding::with_alt(KeyCode::Down, KeyCode::Char('j')),
            navigate_up:      KeyBinding::with_alt(KeyCode::Up, KeyCode::Char('k')),
            play:             KeyBinding::new(KeyCode::Enter),
            stop:             KeyBinding::new(KeyCode::Char('s')),
            volume_up:        KeyBinding::with_alt(KeyCode::Char('+'), KeyCode::Char('=')),
            volume_down:      KeyBinding::new(KeyCode::Char('-')),
            search:           KeyBinding::new(KeyCode::Char('/')),
            toggle_favorite:  KeyBinding::new(KeyCode::Char('f')),
            station_detail:   KeyBinding::new(KeyCode::Char('i')),
            load_more:        KeyBinding::new(KeyCode::Char('n')),
            cycle_panel:      KeyBinding::new(KeyCode::Tab),
            genre_next:       KeyBinding::new(KeyCode::Char(']')),
            genre_prev:       KeyBinding::new(KeyCode::Char('[')),
            genre_picker:     KeyBinding::new(KeyCode::Char('g')),
            theme_picker:     KeyBinding::new(KeyCode::Char('t')),
            visualizer_toggle: KeyBinding::new(KeyCode::Char('v')),
            help:             KeyBinding::new(KeyCode::Char('?')),
            perf_toggle:      KeyBinding::new(KeyCode::Char('`')),
            perf_tick_slower:  KeyBinding::with_alt(KeyCode::Char('<'), KeyCode::Char(',')),
            perf_tick_faster:  KeyBinding::with_alt(KeyCode::Char('>'), KeyCode::Char('.')),
            settings:         KeyBinding::new(KeyCode::Char('S')),
            quit:             KeyBinding::new(KeyCode::Char('q')),
            toggle_source:    KeyBinding::new(KeyCode::Char('m')),
            back:             KeyBinding::with_alt(KeyCode::Backspace, KeyCode::Esc),
            pause:            KeyBinding::new(KeyCode::Char(' ')),
            next_track:       KeyBinding::new(KeyCode::Char('.')),
            prev_track:       KeyBinding::new(KeyCode::Char(',')),
            seek_forward:     KeyBinding::new(KeyCode::Right),
            seek_back:        KeyBinding::new(KeyCode::Left),
        }
    }
}

/// Format a KeyCode as a human-readable string
pub fn keycode_to_string(key: KeyCode) -> String {
    match key {
        KeyCode::Char(c) => match c {
            ' ' => "Space".to_string(),
            _ => c.to_string(),
        },
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Insert => "Ins".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => "???".to_string(),
    }
}

/// Parse a string back into a KeyCode
fn string_to_keycode(s: &str) -> Option<KeyCode> {
    match s {
        "Space" => Some(KeyCode::Char(' ')),
        "Enter" => Some(KeyCode::Enter),
        "Tab" => Some(KeyCode::Tab),
        "Shift+Tab" => Some(KeyCode::BackTab),
        "Backspace" => Some(KeyCode::Backspace),
        "Esc" => Some(KeyCode::Esc),
        "Up" | "↑" => Some(KeyCode::Up),
        "Down" | "↓" => Some(KeyCode::Down),
        "Left" | "←" => Some(KeyCode::Left),
        "Right" | "→" => Some(KeyCode::Right),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PgUp" => Some(KeyCode::PageUp),
        "PgDn" => Some(KeyCode::PageDown),
        "Del" => Some(KeyCode::Delete),
        "Ins" => Some(KeyCode::Insert),
        s if s.starts_with('F') => s[1..].parse::<u8>().ok().map(KeyCode::F),
        s if s.chars().count() == 1 => Some(KeyCode::Char(s.chars().next().unwrap())),
        _ => None,
    }
}

/// Format a KeyBinding as a display string like "↓ / j"
pub fn binding_display(binding: &KeyBinding) -> String {
    let primary = keycode_to_string(binding.primary);
    match binding.alt {
        Some(alt) => format!("{} / {}", primary, keycode_to_string(alt)),
        None => primary,
    }
}

pub struct Config {
    pub tick_rate_ms: u64,
    pub volume: u32,
    /// ISO 3166-1 Alpha-2 country code (e.g. "US", "DE", "GB").
    /// When set, ~30% of station results are blended from this country.
    /// Empty string means no local blending (global results only).
    pub country_code: String,
    pub keybindings: KeyBindings,
    /// Theme name (e.g. "CRT", "Gruvbox", "Nord")
    pub theme: String,
    /// Whether the visualizer is enabled (default: true)
    pub visualizer_enabled: bool,
    /// Which panel is active on startup ("Stations", "Favorites", or "History")
    pub default_panel: String,
    /// When true, theme backgrounds are cleared to let the terminal's own
    /// background (and its transparency, if configured) show through.
    pub transparent_bg: bool,
    /// Subsonic server connection (the "subsonic" object). Holds a
    /// password, which is why save() restricts the file to its owner.
    pub subsonic: SubsonicConfig,
    path: PathBuf,
}

impl Config {
    fn storage_path() -> PathBuf {
        let mut path = crate::storage::paths::base_dir();
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::storage_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                let tick_rate_ms = Self::extract_u64(&contents, "tick_rate_ms")
                    .unwrap_or(DEFAULT_TICK_RATE_MS)
                    .clamp(10, 200);
                let volume = Self::extract_u64(&contents, "volume")
                    .unwrap_or(DEFAULT_VOLUME as u64)
                    .clamp(0, 100) as u32;
                let country_code = Self::extract_string(&contents, "country_code")
                    .unwrap_or_default();
                let keybindings = Self::load_keybindings(&contents);
                let theme = Self::extract_string(&contents, "theme")
                    .unwrap_or_else(|| "CRT".to_string());
                let visualizer_enabled = Self::extract_bool(&contents, "visualizer_enabled")
                    .unwrap_or(true);
                let default_panel = Self::extract_string(&contents, "default_panel")
                    .unwrap_or_else(|| "Stations".to_string());
                let transparent_bg = Self::extract_bool(&contents, "transparent_bg")
                    .unwrap_or(false);
                let subsonic = Self::load_subsonic(&contents);
                return Self { tick_rate_ms, volume, country_code, keybindings, theme, visualizer_enabled, default_panel, transparent_bg, subsonic, path };
            }
        }
        Self {
            tick_rate_ms: DEFAULT_TICK_RATE_MS,
            volume: DEFAULT_VOLUME,
            country_code: String::new(),
            keybindings: KeyBindings::default(),
            theme: "CRT".to_string(),
            visualizer_enabled: true,
            default_panel: "Stations".to_string(),
            transparent_bg: false,
            subsonic: SubsonicConfig::default(),
            path,
        }
    }

    pub fn save(&self) {
        let cc_escaped = escape_json(&self.country_code);

        // Build keybindings JSON
        let mut kb_lines = Vec::new();
        let defaults = KeyBindings::default();
        for (key, _, binding) in self.keybindings.all_actions() {
            // Find the default for comparison
            let default_binding = defaults.all_actions().iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, _, b)| *b);

            // Only save non-default bindings to keep the config clean
            let is_default = default_binding.map_or(false, |d| {
                d.primary == binding.primary && d.alt == binding.alt
            });

            if !is_default {
                let primary_str = keycode_to_string(binding.primary);
                match binding.alt {
                    Some(alt) => {
                        let alt_str = keycode_to_string(alt);
                        kb_lines.push(format!(
                            "      \"{}\": [\"{}\", \"{}\"]",
                            key, primary_str, alt_str
                        ));
                    }
                    None => {
                        kb_lines.push(format!(
                            "      \"{}\": [\"{}\"]",
                            key, primary_str
                        ));
                    }
                }
            }
        }

        let kb_json = if kb_lines.is_empty() {
            "{}".to_string()
        } else {
            format!("{{\n{}\n    }}", kb_lines.join(",\n"))
        };

        let theme_escaped = escape_json(&self.theme);
        let default_panel_escaped = escape_json(&self.default_panel);

        let subsonic_json = format!(
            "{{\n    \"server_url\": \"{}\",\n    \"username\": \"{}\",\n    \"password\": \"{}\"\n  }}",
            escape_json(&self.subsonic.server_url),
            escape_json(&self.subsonic.username),
            escape_json(&self.subsonic.password),
        );

        let json = format!(
            "{{\n  \"tick_rate_ms\": {},\n  \"volume\": {},\n  \"country_code\": \"{}\",\n  \"theme\": \"{}\",\n  \"visualizer_enabled\": {},\n  \"default_panel\": \"{}\",\n  \"transparent_bg\": {},\n  \"subsonic\": {},\n    \"keybindings\": {}\n}}",
            self.tick_rate_ms, self.volume, cc_escaped, theme_escaped, self.visualizer_enabled, default_panel_escaped, self.transparent_bg, subsonic_json, kb_json
        );
        let _ = write_private(&self.path, &json);
    }

    /// Reads the "subsonic" object. Keys are looked up from the start of
    /// that object so its generic names ("username", "password") can't
    /// collide with anything earlier in the file.
    fn load_subsonic(json: &str) -> SubsonicConfig {
        let Some(start) = json.find("\"subsonic\"") else {
            return SubsonicConfig::default();
        };
        let section = &json[start..];
        SubsonicConfig {
            server_url: Self::extract_string(section, "server_url").unwrap_or_default(),
            username: Self::extract_string(section, "username").unwrap_or_default(),
            password: Self::extract_string(section, "password").unwrap_or_default(),
        }
    }

    /// Parse keybindings from the JSON contents, falling back to defaults
    fn load_keybindings(json: &str) -> KeyBindings {
        let mut bindings = KeyBindings::default();

        // Find the "keybindings" object
        let kb_start = match json.find("\"keybindings\"") {
            Some(idx) => idx,
            None => return bindings,
        };
        let after = &json[kb_start..];
        let obj_start = match after.find('{') {
            Some(idx) => kb_start + idx,
            None => return bindings,
        };

        // Find the matching closing brace (simple depth tracking)
        let mut depth = 0;
        let mut obj_end = obj_start;
        for (i, ch) in json[obj_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        obj_end = obj_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let kb_json = &json[obj_start..obj_end];

        // For each action, try to extract its binding
        let action_keys: Vec<&str> = bindings.all_actions().iter()
            .map(|(k, _, _)| *k).collect();

        for key in action_keys {
            if let Some(keys) = Self::extract_key_array(kb_json, key) {
                if let Some(primary) = keys.first().and_then(|s| string_to_keycode(s)) {
                    let alt = keys.get(1).and_then(|s| string_to_keycode(s));
                    bindings.set_binding(key, primary, alt);
                }
            }
        }

        bindings
    }

    /// Extract a JSON array of strings for a key, e.g. "navigate_down": ["↓", "j"]
    fn extract_key_array(json: &str, key: &str) -> Option<Vec<String>> {
        let pattern = format!("\"{}\"", key);
        let idx = json.find(&pattern)?;
        let after = json[idx + pattern.len()..].trim_start();
        let after = after.strip_prefix(':')?.trim_start();
        if !after.starts_with('[') {
            return None;
        }
        let rest = &after[1..];
        let bracket_end = rest.find(']')?;
        let array_content = &rest[..bracket_end];

        let mut result = Vec::new();
        for item in array_content.split(',') {
            let item = item.trim();
            if item.starts_with('"') && item.ends_with('"') {
                result.push(item[1..item.len()-1].to_string());
            }
        }
        if result.is_empty() { None } else { Some(result) }
    }

    /// Extract a numeric value for a given key from simple JSON
    fn extract_u64(json: &str, key: &str) -> Option<u64> {
        let pattern = format!("\"{}\"", key);
        let idx = json.find(&pattern)?;
        let after = json[idx + pattern.len()..].trim_start();
        let after = after.strip_prefix(':')?.trim_start();
        let num_str: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        num_str.parse().ok()
    }

    /// Extract a string value for a given key from simple JSON
    fn extract_string(json: &str, key: &str) -> Option<String> {
        let pattern = format!("\"{}\":", key);
        let idx = json.find(&pattern)?;
        let after = json[idx + pattern.len()..].trim_start();
        if !after.starts_with('"') {
            return None;
        }
        let rest = &after[1..];
        let mut result = String::new();
        let mut chars = rest.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => return Some(result),
                '\\' => {
                    if let Some(escaped) = chars.next() {
                        match escaped {
                            '"' => result.push('"'),
                            '\\' => result.push('\\'),
                            'n' => result.push('\n'),
                            'r' => result.push('\r'),
                            't' => result.push('\t'),
                            _ => result.push(escaped),
                        }
                    }
                }
                _ => result.push(ch),
            }
        }
        None
    }

    fn extract_bool(json: &str, key: &str) -> Option<bool> {
        let pattern = format!("\"{}\":", key);
        let idx = json.find(&pattern)?;
        let after = json[idx + pattern.len()..].trim_start();
        if after.starts_with("true") {
            Some(true)
        } else if after.starts_with("false") {
            Some(false)
        } else {
            None
        }
    }
}

/// Escapes a value for embedding in a JSON string literal.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Writes the file readable by its owner only (Unix), since config.json
/// can hold the Subsonic password. Also tightens files created before
/// that was the case. On Windows the file inherits the user profile ACLs.
fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_u64_basic() {
        let json = r#"{ "tick_rate_ms": 30, "volume": 75 }"#;
        assert_eq!(Config::extract_u64(json, "tick_rate_ms"), Some(30));
        assert_eq!(Config::extract_u64(json, "volume"), Some(75));
    }

    #[test]
    fn test_extract_u64_missing_key() {
        let json = r#"{ "tick_rate_ms": 30 }"#;
        assert_eq!(Config::extract_u64(json, "volume"), None);
    }

    #[test]
    fn test_extract_u64_with_whitespace() {
        let json = r#"{ "tick_rate_ms" :   50 }"#;
        assert_eq!(Config::extract_u64(json, "tick_rate_ms"), Some(50));
    }

    #[test]
    fn test_extract_string_basic() {
        let json = r#"{ "country_code": "US" }"#;
        assert_eq!(Config::extract_string(json, "country_code"), Some("US".to_string()));
    }

    #[test]
    fn test_extract_string_empty() {
        let json = r#"{ "country_code": "" }"#;
        assert_eq!(Config::extract_string(json, "country_code"), Some(String::new()));
    }

    #[test]
    fn test_extract_string_missing() {
        let json = r#"{ "tick_rate_ms": 30 }"#;
        assert_eq!(Config::extract_string(json, "country_code"), None);
    }

    #[test]
    fn test_extract_string_with_full_config() {
        let json = r#"{ "tick_rate_ms": 30, "volume": 50, "country_code": "DE" }"#;
        assert_eq!(Config::extract_u64(json, "tick_rate_ms"), Some(30));
        assert_eq!(Config::extract_u64(json, "volume"), Some(50));
        assert_eq!(Config::extract_string(json, "country_code"), Some("DE".to_string()));
    }

    #[test]
    fn test_subsonic_round_trips_special_characters() {
        let password = r#"p@ss "quoted" \ {braces} , tab	end"#;
        let json = format!(
            r#"{{ "theme": "CRT", "subsonic": {{ "server_url": "{}", "username": "{}", "password": "{}" }}, "keybindings": {{}} }}"#,
            escape_json("http://192.168.1.10:4533"),
            escape_json("me"),
            escape_json(password),
        );
        let subsonic = Config::load_subsonic(&json);
        assert_eq!(subsonic.server_url, "http://192.168.1.10:4533");
        assert_eq!(subsonic.username, "me");
        assert_eq!(subsonic.password, password);
    }

    #[test]
    fn test_subsonic_missing_section_is_empty() {
        let json = r#"{ "tick_rate_ms": 30, "username": "not-subsonic" }"#;
        let subsonic = Config::load_subsonic(json);
        assert_eq!(subsonic, SubsonicConfig::default());
        assert!(!subsonic.is_complete());
    }

    #[cfg(unix)]
    #[test]
    fn test_write_private_restricts_existing_file() {
        use std::os::unix::fs::PermissionsExt;
        let mut path = std::env::temp_dir();
        path.push(format!("aethertune-config-test-{}.json", std::process::id()));
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, "{ }").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ }");
        let _ = fs::remove_file(&path);
    }
}
