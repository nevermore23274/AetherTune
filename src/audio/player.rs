#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(unix)]
use crate::audio::pipe as audio_pipe;
use crate::audio::pipe::SharedAnalysis;

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

pub struct Player {
    /// mpv process for actual audio playback
    process: Option<std::process::Child>,
    /// parec process that captures the PulseAudio monitor for visualization (Unix only)
    #[cfg(unix)]
    capture: Option<std::process::Child>,
    /// FIFO reader thread (Unix only)
    #[cfg(unix)]
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// FIFO path for parec -> reader communication (Unix only)
    #[cfg(unix)]
    fifo_path: Option<PathBuf>,
    socket_path: PathBuf,
    /// IPC stream to mpv (Unix socket on Linux)
    #[cfg(unix)]
    stream: Option<UnixStream>,
    pub analysis: SharedAnalysis,
    /// Legacy audio level for fallback mode (no parec)
    pub audio_level: f64,
    pub media_title: Option<String>,
    request_counter: u64,
    /// Whether parec is available for real audio capture (always false on Windows)
    has_parec: bool,
    /// Real-time stream information from mpv
    pub stream_info: StreamInfo,
    /// Windows Job Object handle — kills mpv automatically if AetherTune is closed via X button
    #[cfg(windows)]
    job_handle: Option<windows_sys::Win32::Foundation::HANDLE>,
}

impl Player {
    pub fn new(analysis: SharedAnalysis) -> Self {
        let socket_path =
            std::env::temp_dir().join(format!("aethertune-mpv-{}", std::process::id()));

        #[cfg(unix)]
        let has_parec = std::process::Command::new("parec")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();

        #[cfg(windows)]
        let has_parec = false;

        #[cfg(windows)]
        let job_handle = create_kill_on_close_job();

        Self {
            process: None,
            #[cfg(unix)]
            capture: None,
            #[cfg(unix)]
            reader_handle: None,
            #[cfg(unix)]
            fifo_path: None,
            socket_path,
            #[cfg(unix)]
            stream: None,
            analysis,
            audio_level: 0.0,
            media_title: None,
            request_counter: 0,
            has_parec,
            stream_info: StreamInfo::new(),
            #[cfg(windows)]
            job_handle,
        }
    }

    /// Returns true if we have real audio analysis running
    pub fn has_real_audio(&self) -> bool {
        #[cfg(unix)]
        { self.capture.is_some() }
        #[cfg(windows)]
        { false }
    }

    pub fn play_url(&mut self, url: &str, volume: u32) {
        self.stop();

        let mut cmd = std::process::Command::new("mpv");
        cmd.arg(url)
            .arg("--no-video")
            .arg(format!("--volume={}", volume))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // On Unix, set up IPC socket for metadata and control
        #[cfg(unix)]
        {
            let socket_str = self.socket_path.to_string_lossy().to_string();
            cmd.arg(format!("--input-ipc-server={}", socket_str));
        }

        match cmd.spawn() {
            Ok(c) => {
                // On Windows, assign mpv to our Job Object so it gets killed
                // automatically if the terminal window is closed via the X button.
                #[cfg(windows)]
                {
                    if let Some(job) = self.job_handle {
                        assign_process_to_job(job, c.as_raw_handle());
                    }
                }

                self.process = Some(c);
                self.media_title = None;
                self.stream_info.reset();
                self.stream_info.stream_connected_at = Some(std::time::Instant::now());

                #[cfg(unix)]
                {
                    // Give mpv a moment to start and create the IPC socket
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    self.connect_ipc();

                    // Start audio capture for visualization if parec is available
                    if self.has_parec {
                        self.start_capture();
                    }
                }
            }
            Err(e) => eprintln!("Failed to start mpv: {}", e),
        }
    }

    /// Start parec to capture the PulseAudio/PipeWire monitor source.
    #[cfg(unix)]
    fn start_capture(&mut self) {
        let fifo = audio_pipe::fifo_path();

        if !audio_pipe::create_fifo(&fifo) {
            return;
        }

        let fifo_str = fifo.to_string_lossy().to_string();

        // Spawn the FIFO reader thread first (it blocks on open until parec writes)
        let reader_handle = audio_pipe::spawn_reader(fifo.clone(), self.analysis.clone());

        let capture = unsafe {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "exec parec --format=s16le --channels=2 --rate=48000 \
                     --device=$(pactl get-default-sink).monitor > {}",
                    shell_escape(&fifo_str)
                ))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .pre_exec(|| {
                    libc::setsid();
                    Ok(())
                })
                .spawn()
        };

        match capture {
            Ok(c) => {
                self.capture = Some(c);
                self.reader_handle = Some(reader_handle);
                self.fifo_path = Some(fifo);
            }
            Err(_) => {
                audio_pipe::cleanup_fifo(&fifo);
            }
        }
    }

    #[cfg(unix)]
    fn stop_capture(&mut self) {
        if let Some(mut cap) = self.capture.take() {
            unsafe {
                libc::kill(-(cap.id() as i32), libc::SIGTERM);
            }
            let _ = cap.kill();
            let _ = cap.wait();
        }

        if let Some(ref fifo) = self.fifo_path.take() {
            audio_pipe::cleanup_fifo(fifo);
        }

        if let Some(handle) = self.reader_handle.take() {
            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }
                if start.elapsed() > std::time::Duration::from_millis(200) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        if let Ok(mut a) = self.analysis.lock() {
            a.active = false;
            a.rms = 0.0;
            a.bands = [0.0; crate::audio::pipe::NUM_BANDS];
        }
    }

    #[cfg(windows)]
    fn stop_capture(&mut self) {
        // No capture on Windows yet
    }

    #[cfg(unix)]
    fn connect_ipc(&mut self) {
        for _ in 0..5 {
            match UnixStream::connect(&self.socket_path) {
                Ok(stream) => {
                    stream.set_nonblocking(true).ok();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_millis(5)))
                        .ok();
                    self.stream = Some(stream);

                    self.send_command(
                        r#"{ "command": ["observe_property", 1, "media-title"] }"#,
                    );
                    self.send_command(
                        r#"{ "command": ["observe_property", 2, "audio-codec-name"] }"#,
                    );
                    self.send_command(
                        r#"{ "command": ["observe_property", 3, "audio-params/samplerate"] }"#,
                    );
                    self.send_command(
                        r#"{ "command": ["observe_property", 4, "audio-params/channel-count"] }"#,
                    );
                    return;
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
    }

    #[cfg(unix)]
    fn send_command(&mut self, command: &str) {
        if let Some(ref mut stream) = self.stream {
            let msg = format!("{}\n", command);
            if stream.write_all(msg.as_bytes()).is_err() {
                self.stream = None;
            }
        }
    }

    #[cfg(windows)]
    fn send_command(&mut self, _command: &str) {
        // No IPC on Windows yet
    }

    pub fn set_volume(&mut self, volume: u32) {
        let cmd = format!(
            "{{ \"command\": [\"set_property\", \"volume\", {}] }}",
            volume
        );
        self.send_command(&cmd);
    }

    pub fn poll(&mut self) {
        self.request_counter += 1;

        #[cfg(windows)]
        return;

        #[cfg(unix)]
        {
            // In fallback mode (no parec), poll audio-pts for activity detection
            if !self.has_real_audio() {
                if self.request_counter % 3 == 0 {
                    self.send_command(
                        r#"{ "command": ["get_property", "audio-pts"], "request_id": 100 }"#,
                    );
                }
            }

            // Poll stream info properties periodically (every ~5 ticks)
            if self.request_counter % 5 == 0 {
                self.send_command(
                    r#"{ "command": ["get_property", "audio-bitrate"], "request_id": 200 }"#,
                );
                self.send_command(
                    r#"{ "command": ["get_property", "demuxer-cache-duration"], "request_id": 201 }"#,
                );
            }

            if self.stream.is_none() {
                return;
            }

            let stream = match self.stream.as_ref().and_then(|s| s.try_clone().ok()) {
                Some(s) => s,
                None => return,
            };

            let reader = BufReader::new(stream);
            let mut new_title: Option<String> = None;
            let mut got_audio_pts = false;

            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if let Some(title) = Self::extract_media_title(&text) {
                            if !title.is_empty() {
                                new_title = Some(title);
                            }
                        }

                        if text.contains("\"request_id\":100") || text.contains("\"request_id\": 100")
                        {
                            if text.contains("\"data\":") && !text.contains("\"error\"") {
                                got_audio_pts = true;
                            }
                        }

                        self.parse_stream_info(&text);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }

            if let Some(title) = new_title {
                self.media_title = Some(title);
            }

            if !self.has_real_audio() && got_audio_pts {
                self.audio_level = 0.7;
            }
        }
    }

    fn parse_stream_info(&mut self, text: &str) {
        if text.contains("\"request_id\":200") || text.contains("\"request_id\": 200") {
            if let Some(val) = Self::extract_number(text) {
                self.stream_info.audio_bitrate = val;
            }
        }
        if text.contains("\"request_id\":201") || text.contains("\"request_id\": 201") {
            if let Some(val) = Self::extract_number(text) {
                self.stream_info.cache_duration = val;
            }
        }
        if text.contains("\"id\":2") || text.contains("\"id\": 2") {
            if let Some(val) = Self::extract_string_value(text) {
                self.stream_info.audio_codec = val;
            }
        }
        if text.contains("\"id\":3") || text.contains("\"id\": 3") {
            if let Some(val) = Self::extract_number(text) {
                self.stream_info.sample_rate = val as u32;
            }
        }
        if text.contains("\"id\":4") || text.contains("\"id\": 4") {
            if let Some(val) = Self::extract_number(text) {
                self.stream_info.channels = val as u32;
            }
        }
    }

    fn extract_number(json: &str) -> Option<f64> {
        let data_key = "\"data\":";
        let idx = json.find(data_key)?;
        let after = json[idx + data_key.len()..].trim_start();
        let num_str: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect();
        num_str.parse::<f64>().ok()
    }

    fn extract_string_value(json: &str) -> Option<String> {
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

    fn extract_media_title(json_line: &str) -> Option<String> {
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

    pub fn stop(&mut self) {
        #[cfg(unix)]
        { self.stream = None; }

        // Stop audio capture first
        self.stop_capture();

        // Then stop mpv
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        let _ = std::fs::remove_file(&self.socket_path);
        self.audio_level = 0.0;
        self.media_title = None;
        self.stream_info.reset();
    }

    pub fn is_playing(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();

        #[cfg(windows)]
        {
            if let Some(job) = self.job_handle.take() {
                unsafe { windows_sys::Win32::Foundation::CloseHandle(job) };
            }
        }
    }
}

#[cfg(unix)]
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Create a Windows Job Object configured to kill all assigned processes
/// when the last handle to the job is closed (i.e. when AetherTune exits).
#[cfg(windows)]
fn create_kill_on_close_job() -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::System::JobObjects::*;

    // Define locally to avoid needing extra windows-sys feature flags for IO_COUNTERS
    #[repr(C)]
    struct ExtendedLimitInfo {
        basic: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: [u8; 48], // IO_COUNTERS padding
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }

        let mut info: ExtendedLimitInfo = std::mem::zeroed();
        info.basic.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<ExtendedLimitInfo>() as u32,
        );

        if ok == 0 {
            windows_sys::Win32::Foundation::CloseHandle(job);
            return None;
        }

        Some(job)
    }
}

/// Assign a spawned process to a Job Object so it inherits the kill-on-close behavior.
#[cfg(windows)]
fn assign_process_to_job(
    job: windows_sys::Win32::Foundation::HANDLE,
    process: std::os::windows::io::RawHandle,
) {
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

    unsafe {
        AssignProcessToJobObject(job, process as *mut std::ffi::c_void);
    }
}