//! Parsing for mpv's JSON IPC line protocol.
//!
//! This is pure string-in/data-out logic with no dependency on how the
//! lines arrived — player.rs feeds it lines read off a Unix socket on
//! Linux/macOS or off a named-pipe reader thread on Windows, and both
//! paths call the same functions here.

use serde_json::{json, Value};

pub struct StreamInfo {
    /// Actual audio bitrate in bits/sec from mpv (0 if unknown)
    pub audio_bitrate: f64,
    /// Audio codec name reported by mpv
    pub audio_codec: String,
    /// Demuxer cache duration in seconds (how much audio is buffered)
    pub cache_duration: f64,
    /// How long the current stream has been connected
    pub stream_connected_at: Option<std::time::Instant>,
    /// Audio sample rate from mpv
    pub sample_rate: u32,
    /// Audio channel count
    pub channels: u32,
}

impl StreamInfo {
    pub fn new() -> Self {
        Self {
            audio_bitrate: 0.0,
            audio_codec: String::new(),
            cache_duration: 0.0,
            stream_connected_at: None,
            sample_rate: 0,
            channels: 0,
        }
    }

    pub fn reset(&mut self) {
        self.audio_bitrate = 0.0;
        self.audio_codec.clear();
        self.cache_duration = 0.0;
        self.stream_connected_at = None;
        self.sample_rate = 0;
        self.channels = 0;
    }

    pub fn uptime_str(&self) -> String {
        match self.stream_connected_at {
            Some(t) => {
                let secs = t.elapsed().as_secs();
                if secs >= 3600 {
                    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                } else if secs >= 60 {
                    format!("{}m {}s", secs / 60, secs % 60)
                } else {
                    format!("{}s", secs)
                }
            }
            None => "—".to_string(),
        }
    }
}

/// Update `info` from one line of mpv IPC JSON, if it's a reply this
/// module recognizes (request_id 200/201, or observe_property ids 2-4).
pub fn parse_stream_info(info: &mut StreamInfo, text: &str) {
    if text.contains("\"request_id\":200") || text.contains("\"request_id\": 200") {
        if let Some(val) = extract_number(text) {
            info.audio_bitrate = val;
        }
    }
    if text.contains("\"request_id\":201") || text.contains("\"request_id\": 201") {
        if let Some(val) = extract_number(text) {
            info.cache_duration = val;
        }
    }
    if text.contains("\"id\":2") || text.contains("\"id\": 2") {
        if let Some(val) = extract_string_value(text) {
            info.audio_codec = val;
        }
    }
    if text.contains("\"id\":3") || text.contains("\"id\": 3") {
        if let Some(val) = extract_number(text) {
            info.sample_rate = val as u32;
        }
    }
    if text.contains("\"id\":4") || text.contains("\"id\": 4") {
        if let Some(val) = extract_number(text) {
            info.channels = val as u32;
        }
    }
}

/// request_id for the periodic `get_property time-pos` poll in playlist
/// sessions. Kept clear of the ids parse_stream_info() substring-matches.
pub const TIME_POS_REQUEST_ID: u64 = 300;

/// Playback state for playlist sessions (Player::play_playlist). Radio
/// streams don't populate this — they're a single endless stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaybackState {
    /// Index of the playing playlist entry (None before start / after end).
    pub playlist_pos: Option<usize>,
    /// Seconds into the current entry.
    pub time_pos: Option<f64>,
    /// Length of the current entry in seconds, once mpv knows it.
    pub duration: Option<f64>,
    pub paused: bool,
    /// mpv is idle: nothing loaded/playing.
    pub idle: bool,
    /// Set once any entry has started, so idle-at-startup isn't mistaken
    /// for the playlist having run out.
    pub started: bool,
}

impl PlaybackState {
    /// True when the whole playlist has played through.
    pub fn finished(&self) -> bool {
        self.started && self.idle
    }
}

/// Update `state` from one line of mpv IPC JSON. Unlike the substring
/// matching above, this parses the line properly and matches events by
/// property name, so it can't confuse e.g. id 2 with id 20.
pub fn parse_playback_state(state: &mut PlaybackState, text: &str) {
    let Ok(line) = serde_json::from_str::<Value>(text) else { return };
    let data = line.get("data");

    if line.get("event").and_then(Value::as_str) == Some("property-change") {
        match line.get("name").and_then(Value::as_str) {
            Some("playlist-pos") => {
                let pos = data.and_then(Value::as_i64).filter(|p| *p >= 0).map(|p| p as usize);
                if pos != state.playlist_pos {
                    // Position/length belong to the previous entry
                    state.time_pos = None;
                    state.duration = None;
                }
                state.playlist_pos = pos;
                if pos.is_some() {
                    state.started = true;
                }
            }
            Some("pause") => state.paused = data.and_then(Value::as_bool).unwrap_or(false),
            Some("idle-active") => state.idle = data.and_then(Value::as_bool).unwrap_or(false),
            Some("duration") => state.duration = data.and_then(Value::as_f64),
            _ => {}
        }
    } else if line.get("request_id").and_then(Value::as_u64) == Some(TIME_POS_REQUEST_ID) {
        // Errors (e.g. "property unavailable" between entries) leave data absent
        state.time_pos = data.and_then(Value::as_f64);
    }
}

/// Serializes an mpv IPC command, JSON-escaping every argument (URLs can
/// contain quotes or backslashes).
pub fn command(args: &[Value]) -> String {
    json!({ "command": args }).to_string()
}

pub fn extract_number(json: &str) -> Option<f64> {
    let data_key = "\"data\":";
    let idx = json.find(data_key)?;
    let after = json[idx + data_key.len()..].trim_start();
    let num_str: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num_str.parse::<f64>().ok()
}

pub fn extract_string_value(json: &str) -> Option<String> {
    let data_key = "\"data\":";
    let idx = json.find(data_key)?;
    let after = json[idx + data_key.len()..].trim_start();
    if after.starts_with('"') {
        let rest = &after[1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        None
    }
}

pub fn extract_media_title(json_line: &str) -> Option<String> {
    if !json_line.contains("media-title") {
        return None;
    }

    let data_key = "\"data\":";
    let idx = json_line.find(data_key)?;
    let after = &json_line[idx + data_key.len()..];
    let trimmed = after.trim_start();

    if trimmed.starts_with('"') {
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
                            'n' => result.push(' '),
                            _ => result.push(escaped),
                        }
                    }
                }
                _ => result.push(ch),
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;

    fn feed(state: &mut PlaybackState, lines: &[&str]) {
        for line in lines {
            parse_playback_state(state, line);
        }
    }

    #[test]
    fn playlist_lifecycle() {
        let mut state = PlaybackState::default();
        feed(&mut state, &[
            r#"{"event":"property-change","id":7,"name":"idle-active","data":true}"#,
            r#"{"event":"property-change","id":5,"name":"playlist-pos","data":-1}"#,
        ]);
        // Idle before anything played is not "finished"
        assert!(!state.finished());

        feed(&mut state, &[
            r#"{"event":"property-change","id":7,"name":"idle-active","data":false}"#,
            r#"{"event":"property-change","id":5,"name":"playlist-pos","data":0}"#,
            r#"{"event":"property-change","id":8,"name":"duration","data":245.3}"#,
            r#"{"request_id":300,"error":"success","data":12.5}"#,
        ]);
        assert_eq!(state.playlist_pos, Some(0));
        assert_eq!(state.duration, Some(245.3));
        assert_eq!(state.time_pos, Some(12.5));

        // Track change clears the old entry's position and length
        feed(&mut state, &[r#"{"event":"property-change","id":5,"name":"playlist-pos","data":1}"#]);
        assert_eq!(state.playlist_pos, Some(1));
        assert_eq!(state.time_pos, None);
        assert_eq!(state.duration, None);

        feed(&mut state, &[
            r#"{"event":"property-change","id":5,"name":"playlist-pos","data":-1}"#,
            r#"{"event":"property-change","id":7,"name":"idle-active","data":true}"#,
        ]);
        assert_eq!(state.playlist_pos, None);
        assert!(state.finished());
    }

    #[test]
    fn pause_and_errors() {
        let mut state = PlaybackState { time_pos: Some(3.0), ..Default::default() };
        feed(&mut state, &[
            r#"{"event":"property-change","id":6,"name":"pause","data":true}"#,
            r#"{"request_id":300,"error":"property unavailable"}"#,
            r#"{"request_id":200,"error":"success","data":128000}"#,
            "not json",
        ]);
        assert!(state.paused);
        assert_eq!(state.time_pos, None);
    }

    #[test]
    fn command_escapes_arguments() {
        let cmd = command(&[json!("loadfile"), json!(r#"http://h/a?x="q"\"#), json!("append")]);
        let parsed: Value = serde_json::from_str(&cmd).unwrap();
        assert_eq!(parsed["command"][1], r#"http://h/a?x="q"\"#);
    }
}
