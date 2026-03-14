use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FavoriteEntry {
    pub name: String,
    pub url: String,
    pub genre: String,
    pub country: String,
    pub bitrate: u32,
}

pub struct FavoritesStore {
    pub entries: Vec<FavoriteEntry>,
    path: PathBuf,
}

impl FavoritesStore {
    fn storage_path() -> PathBuf {
        let base = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(base);
        path.push(".aethertune");
        fs::create_dir_all(&path).ok();
        path.push("favorites.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::storage_path();
        let entries = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| Self::parse_entries(&s))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self { entries, path }
    }

    pub fn save(&self) {
        let json = Self::serialize_entries(&self.entries);
        let _ = fs::write(&self.path, json);
    }

    /// Toggles a station in favorites. Returns true if added, false if removed.
    pub fn toggle(
        &mut self,
        name: &str,
        url: &str,
        genre: &str,
        country: &str,
        bitrate: u32,
    ) -> bool {
        if let Some(idx) = self.entries.iter().position(|f| f.url == url) {
            self.entries.remove(idx);
            self.save();
            false
        } else {
            self.entries.push(FavoriteEntry {
                name: name.to_string(),
                url: url.to_string(),
                genre: genre.to_string(),
                country: country.to_string(),
                bitrate,
            });
            self.save();
            true
        }
    }

    fn escape_json(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    fn serialize_entries(entries: &[FavoriteEntry]) -> String {
        let mut out = String::from("[\n");
        for (i, e) in entries.iter().enumerate() {
            out.push_str("  {\n");
            out.push_str(&format!("    \"name\": \"{}\",\n", Self::escape_json(&e.name)));
            out.push_str(&format!("    \"url\": \"{}\",\n", Self::escape_json(&e.url)));
            out.push_str(&format!("    \"genre\": \"{}\",\n", Self::escape_json(&e.genre)));
            out.push_str(&format!("    \"country\": \"{}\",\n", Self::escape_json(&e.country)));
            out.push_str(&format!("    \"bitrate\": {}\n", e.bitrate));
            out.push_str("  }");
            if i < entries.len() - 1 {
                out.push(',');
            }
            out.push('\n');
        }
        out.push(']');
        out
    }

    fn parse_entries(json: &str) -> Option<Vec<FavoriteEntry>> {
        let trimmed = json.trim();
        if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
            return None;
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut entries = Vec::new();
        let mut depth = 0;
        let mut start = None;

        for (i, ch) in inner.char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            let obj = &inner[s..=i];
                            if let Some(entry) = Self::parse_one_entry(obj) {
                                entries.push(entry);
                            }
                        }
                        start = None;
                    }
                }
                _ => {}
            }
        }
        Some(entries)
    }

    fn parse_one_entry(obj: &str) -> Option<FavoriteEntry> {
        let name = Self::extract_string_field(obj, "name")?;
        let url = Self::extract_string_field(obj, "url")?;
        let genre = Self::extract_string_field(obj, "genre").unwrap_or_default();
        let country = Self::extract_string_field(obj, "country").unwrap_or_default();
        let bitrate = Self::extract_number_field(obj, "bitrate").unwrap_or(0);
        Some(FavoriteEntry { name, url, genre, country, bitrate })
    }

    fn extract_string_field(obj: &str, field: &str) -> Option<String> {
        let pattern = format!("\"{}\"", field);
        let idx = obj.find(&pattern)?;
        let after_key = &obj[idx + pattern.len()..];
        let after_colon = after_key.find(':').map(|i| &after_key[i + 1..])?;
        let trimmed = after_colon.trim_start();
        if !trimmed.starts_with('"') {
            return None;
        }
        let rest = &trimmed[1..];
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
                            _ => {
                                result.push('\\');
                                result.push(escaped);
                            }
                        }
                    }
                }
                _ => result.push(ch),
            }
        }
        None
    }

    fn extract_number_field(obj: &str, field: &str) -> Option<u32> {
        let pattern = format!("\"{}\"", field);
        let idx = obj.find(&pattern)?;
        let after_key = &obj[idx + pattern.len()..];
        let after_colon = after_key.find(':').map(|i| &after_key[i + 1..])?;
        let trimmed = after_colon.trim_start();
        let num_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse().ok()
    }
}