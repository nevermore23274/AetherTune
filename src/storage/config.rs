use std::fs;
use std::path::PathBuf;
use crossterm::event::KeyCode;

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
    pub help: KeyBinding,
    pub perf_toggle: KeyBinding,
    pub perf_tick_slower: KeyBinding,
    pub perf_tick_faster: KeyBinding,
    pub settings: KeyBinding,
    pub quit: KeyBinding,
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
            ("help",             "Help Overlay",        &self.help),
            ("perf_toggle",      "Perf Profiler",       &self.perf_toggle),
            ("perf_tick_slower",  "Tick Rate Slower",   &self.perf_tick_slower),
            ("perf_tick_faster",  "Tick Rate Faster",   &self.perf_tick_faster),
            ("settings",         "Settings",            &self.settings),
            ("quit",             "Quit",                &self.quit),
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
            "help"             => &mut self.help,
            "perf_toggle"      => &mut self.perf_toggle,
            "perf_tick_slower"  => &mut self.perf_tick_slower,
            "perf_tick_faster"  => &mut self.perf_tick_faster,
            "settings"         => &mut self.settings,
            "quit"             => &mut self.quit,
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
            help:             KeyBinding::new(KeyCode::Char('?')),
            perf_toggle:      KeyBinding::new(KeyCode::Char('`')),
            perf_tick_slower:  KeyBinding::with_alt(KeyCode::Char('<'), KeyCode::Char(',')),
            perf_tick_faster:  KeyBinding::with_alt(KeyCode::Char('>'), KeyCode::Char('.')),
            settings:         KeyBinding::new(KeyCode::Char('S')),
            quit:             KeyBinding::new(KeyCode::Char('q')),
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
    path: PathBuf,
}

impl Config {
    fn storage_path() -> PathBuf {
        let base = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(base);
        path.push(".aethertune");
        fs::create_dir_all(&path).ok();
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
                return Self { tick_rate_ms, volume, country_code, keybindings, path };
            }
        }
        Self {
            tick_rate_ms: DEFAULT_TICK_RATE_MS,
            volume: DEFAULT_VOLUME,
            country_code: String::new(),
            keybindings: KeyBindings::default(),
            path,
        }
    }

    pub fn save(&self) {
        let cc_escaped = self.country_code.replace('\\', "\\\\").replace('"', "\\\"");

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

        let json = format!(
            "{{\n  \"tick_rate_ms\": {},\n  \"volume\": {},\n  \"country_code\": \"{}\",\n    \"keybindings\": {}\n}}",
            self.tick_rate_ms, self.volume, cc_escaped, kb_json
        );
        let _ = fs::write(&self.path, json);
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
                            _ => result.push(escaped),
                        }
                    }
                }
                _ => result.push(ch),
            }
        }
        None
    }
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
}