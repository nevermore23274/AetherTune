use std::fs;
use std::path::PathBuf;

const DEFAULT_TICK_RATE_MS: u64 = 30;
const DEFAULT_VOLUME: u32 = 50;

pub struct Config {
    pub tick_rate_ms: u64,
    pub volume: u32,
    /// ISO 3166-1 Alpha-2 country code (e.g. "US", "DE", "GB").
    /// When set, ~30% of station results are blended from this country.
    /// Empty string means no local blending (global results only).
    pub country_code: String,
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
                return Self { tick_rate_ms, volume, country_code, path };
            }
        }
        Self {
            tick_rate_ms: DEFAULT_TICK_RATE_MS,
            volume: DEFAULT_VOLUME,
            country_code: String::new(),
            path,
        }
    }

    pub fn save(&self) {
        let cc_escaped = self.country_code.replace('\\', "\\\\").replace('"', "\\\"");
        let json = format!(
            "{{\n  \"tick_rate_ms\": {},\n  \"volume\": {},\n  \"country_code\": \"{}\"\n}}",
            self.tick_rate_ms, self.volume, cc_escaped
        );
        let _ = fs::write(&self.path, json);
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