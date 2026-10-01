#[cfg(any(unix, windows))]
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

#[cfg(target_os = "linux")]
use crate::audio::pipe as audio_pipe;
use crate::audio::pipe::SharedAnalysis;

#[cfg(any(windows, target_os = "macos"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(any(windows, target_os = "macos"))]
use std::sync::Arc;

#[cfg(any(unix, windows))]
use std::sync::mpsc;

use crate::audio::mpv_ipc::{self, PlaybackState, StreamInfo};
use serde_json::json;

/// Properties observed only in playlist sessions (see play_playlist()).
/// IDs 5-8 avoid the 1-4 used above and the substring matching in
/// mpv_ipc::parse_stream_info(); parse_playback_state() matches by name.
const OBSERVE_PLAYLIST: &str = "{ \"command\": [\"observe_property\", 5, \"playlist-pos\"] }\n\
                                { \"command\": [\"observe_property\", 6, \"pause\"] }\n\
                                { \"command\": [\"observe_property\", 7, \"idle-active\"] }\n\
                                { \"command\": [\"observe_property\", 8, \"duration\"] }";

pub struct Player {
    /// mpv process for actual audio playback
    process: Option<std::process::Child>,
    /// parec process that captures the PulseAudio monitor for visualization (Linux only)
    #[cfg(target_os = "linux")]
    capture: Option<std::process::Child>,
    /// FIFO reader thread (Linux only)
    #[cfg(target_os = "linux")]
    reader_handle: Option<std::thread::JoinHandle<()>>,
    /// FIFO path for parec -> reader communication (Linux only)
    #[cfg(target_os = "linux")]
    fifo_path: Option<PathBuf>,
    /// WASAPI loopback capture thread (Windows only)
    #[cfg(windows)]
    capture_handle: Option<std::thread::JoinHandle<()>>,
    /// Stop flag for the WASAPI capture thread (Windows only)
    #[cfg(windows)]
    capture_stop: Arc<AtomicBool>,
    /// Core Audio process-tap capture thread (macOS only)
    #[cfg(target_os = "macos")]
    capture_handle: Option<std::thread::JoinHandle<()>>,
    /// Stop flag for the Core Audio capture thread (macOS only)
    #[cfg(target_os = "macos")]
    capture_stop: Arc<AtomicBool>,
    /// Job Object that ensures mpv.exe dies when AetherTune exits (Windows only)
    #[cfg(windows)]
    job_object: Option<crate::audio::jobobject::JobObject>,
    /// Named pipe path passed to mpv's --input-ipc-server (Windows only).
    /// mpv speaks the same JSON IPC protocol over a named pipe here as it
    /// does over a Unix socket on Linux/macOS.
    #[cfg(windows)]
    pipe_name: String,
    /// Write handle to the connected named pipe (Windows only).
    #[cfg(windows)]
    pipe: Option<std::fs::File>,
    /// Background thread doing a blocking read_line() loop over its own
    /// cloned handle to the pipe (Windows only). Named pipes opened via
    /// std::fs::File have no non-blocking read mode without dropping into
    /// overlapped I/O, so — same idiom as the parec/WASAPI capture threads
    /// elsewhere in this file — a dedicated thread does the blocking work
    /// and hands completed lines back over a channel.
    #[cfg(windows)]
    pipe_reader_handle: Option<std::thread::JoinHandle<()>>,
    /// Receiving end of that channel; poll() drains it non-blockingly.
    #[cfg(windows)]
    pipe_rx: Option<mpsc::Receiver<String>>,
    /// Bumped every time stop()/play_url() tears down the current stream.
    /// A background connect thread (see connect_ipc()/connect_pipe())
    /// tags the generation it was started for, so if it finishes after
    /// a newer play_url() call has already superseded it, poll() can
    /// recognize the result as stale and discard it instead of wiring a
    /// dead connection's handles into a live session.
    ipc_generation: u64,
    /// Unix: result of an in-progress connect_ipc() background attempt,
    /// drained non-blockingly by poll().
    #[cfg(unix)]
    ipc_connect_rx: Option<mpsc::Receiver<(u64, UnixStream, BufReader<UnixStream>)>>,
    /// Windows: result of an in-progress connect_pipe() background
    /// attempt, drained non-blockingly by poll().
    #[cfg(windows)]
    ipc_connect_rx:
        Option<mpsc::Receiver<(u64, std::fs::File, std::thread::JoinHandle<()>, mpsc::Receiver<String>)>>,
    socket_path: PathBuf,
    /// IPC stream to mpv (Unix socket on Linux and macOS) — used for writing commands
    #[cfg(unix)]
    stream: Option<UnixStream>,
    /// Buffered reader over a cloned handle to the same IPC socket, kept
    /// alive across ticks so partial lines aren't dropped between polls
    /// and so we're not dup()'ing the fd + allocating a fresh buffer
    /// every tick.
    #[cfg(unix)]
    reader: Option<BufReader<UnixStream>>,
    /// In-progress line for the reader above. `read_line` appends to
    /// whatever's already here, so a line that's only half-arrived when
    /// we hit WouldBlock stays put until the next poll completes it —
    /// this must be a field (not a local in `poll()`) or the partial
    /// bytes are lost the moment the local goes out of scope.
    #[cfg(unix)]
    line_buf: String,
    pub analysis: SharedAnalysis,
    /// Legacy audio level for fallback mode (no real capture backend)
    pub audio_level: f64,
    pub media_title: Option<String>,
    request_counter: u64,
    /// Whether parec is available for real audio capture (Linux only)
    #[cfg(target_os = "linux")]
    has_parec: bool,
    /// Whether this system supports Core Audio process taps, i.e. macOS
    /// 14.4+ (macOS only)
    #[cfg(target_os = "macos")]
    has_coreaudio_tap: bool,
    /// Real-time stream information from mpv
    pub stream_info: StreamInfo,
    /// Whether the visualizer is enabled (controls capture startup)
    pub visualizer_enabled: bool,
    /// True when the current mpv session was started by play_playlist()
    /// rather than play_url(). Gates all playlist-only behaviour so the
    /// radio path is unaffected.
    playlist_mode: bool,
    /// Playlist commands waiting for the IPC connection (playlist mode
    /// only). Radio commands are still dropped while disconnected, as before.
    pending_commands: Vec<String>,
    /// Queue position, track time, pause state (playlist mode only)
    pub playback: PlaybackState,
}

impl Player {
    pub fn new(analysis: SharedAnalysis) -> Self {
        let socket_path =
            std::env::temp_dir().join(format!("aethertune-mpv-{}", std::process::id()));

        #[cfg(target_os = "linux")]
        let has_parec = std::process::Command::new("parec")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();

        #[cfg(target_os = "macos")]
        let has_coreaudio_tap = crate::audio::coreaudio_capture::taps_supported();

        Self {
            process: None,
            #[cfg(target_os = "linux")]
            capture: None,
            #[cfg(target_os = "linux")]
            reader_handle: None,
            #[cfg(target_os = "linux")]
            fifo_path: None,
            #[cfg(windows)]
            capture_handle: None,
            #[cfg(windows)]
            capture_stop: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "macos")]
            capture_handle: None,
            #[cfg(target_os = "macos")]
            capture_stop: Arc::new(AtomicBool::new(false)),
            #[cfg(windows)]
            job_object: crate::audio::jobobject::JobObject::new(),
            #[cfg(windows)]
            pipe_name: format!(r"\\.\pipe\aethertune-mpv-{}", std::process::id()),
            #[cfg(windows)]
            pipe: None,
            #[cfg(windows)]
            pipe_reader_handle: None,
            #[cfg(windows)]
            pipe_rx: None,
            ipc_generation: 0,
            #[cfg(unix)]
            ipc_connect_rx: None,
            #[cfg(windows)]
            ipc_connect_rx: None,
            socket_path,
            #[cfg(unix)]
            stream: None,
            #[cfg(unix)]
            reader: None,
            #[cfg(unix)]
            line_buf: String::new(),
            analysis,
            audio_level: 0.0,
            media_title: None,
            request_counter: 0,
            #[cfg(target_os = "linux")]
            has_parec,
            #[cfg(target_os = "macos")]
            has_coreaudio_tap,
            stream_info: StreamInfo::new(),
            visualizer_enabled: true,
            playlist_mode: false,
            pending_commands: Vec::new(),
            playback: PlaybackState::default(),
        }
    }

    /// Returns true if we have real audio analysis running
    pub fn has_real_audio(&self) -> bool {
        #[cfg(target_os = "linux")]
        { self.capture.is_some() }
        #[cfg(any(windows, target_os = "macos"))]
        { self.capture_handle.is_some() }
        #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
        { false }
    }

    pub fn play_url(&mut self, url: &str, volume: u32) -> bool {
        self.stop();
        self.spawn_mpv(Some(url), volume)
    }

    /// Spawns mpv (with `url`, or idle with no file for playlist sessions),
    /// then starts the IPC connect and audio capture. Shared by play_url()
    /// and play_playlist(); callers stop() first.
    fn spawn_mpv(&mut self, url: Option<&str>, volume: u32) -> bool {
        let mut cmd = std::process::Command::new("mpv");
        match url {
            Some(url) => cmd.arg(url),
            // Stay running with an empty playlist until IPC loads one,
            // and after the playlist ends (PlaybackState::finished()).
            None => cmd.arg("--idle=yes"),
        };
        cmd.arg("--no-video")
            .arg(format!("--volume={}", volume))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // On Unix, set up IPC socket for metadata and control
        #[cfg(unix)]
        {
            let socket_str = self.socket_path.to_string_lossy().to_string();
            cmd.arg(format!("--input-ipc-server={}", socket_str));
        }

        // On Windows, mpv speaks the same JSON IPC protocol over a named
        // pipe instead of a Unix socket.
        #[cfg(windows)]
        {
            cmd.arg(format!("--input-ipc-server={}", self.pipe_name));
        }

        match cmd.spawn() {
            Ok(c) => {
                // On Windows, assign mpv to the Job Object so it dies with us
                #[cfg(windows)]
                {
                    if let Some(ref job) = self.job_object {
                        job.assign(&c);
                    }
                }

                self.process = Some(c);
                self.media_title = None;
                self.stream_info.reset();
                self.stream_info.stream_connected_at = Some(std::time::Instant::now());

                #[cfg(unix)]
                {
                    // connect_ipc() kicks off a background connect and
                    // returns immediately — see its doc comment for why.
                    self.connect_ipc();

                    // Start audio capture for visualization if a capture
                    // backend is available on this platform and the
                    // visualizer is on
                    #[cfg(target_os = "linux")]
                    if self.has_parec && self.visualizer_enabled {
                        self.start_capture();
                    }
                    #[cfg(target_os = "macos")]
                    if self.has_coreaudio_tap && self.visualizer_enabled {
                        self.start_capture();
                    }
                }

                // On Windows, kick off a background connect to mpv's
                // named-pipe IPC (connect_pipe() returns immediately —
                // see its doc comment for why), then start WASAPI
                // loopback capture if visualizer is enabled
                #[cfg(windows)]
                {
                    self.connect_pipe();

                    if self.visualizer_enabled {
                        self.start_capture();
                    }
                }

                true
            }
            Err(_) => false,
        }
    }

    /// Start parec to capture the PulseAudio/PipeWire monitor source.
    #[cfg(target_os = "linux")]
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

    /// Start WASAPI loopback capture for real-time audio visualization.
    #[cfg(windows)]
    fn start_capture(&mut self) {
        // Reset the stop flag and spawn the capture thread
        self.capture_stop.store(false, Ordering::Relaxed);
        let handle = crate::audio::wasapi_capture::spawn_capture_thread(
            self.analysis.clone(),
            self.capture_stop.clone(),
        );
        self.capture_handle = Some(handle);
    }

    /// Start Core Audio process-tap capture for real-time audio visualization.
    #[cfg(target_os = "macos")]
    fn start_capture(&mut self) {
        // Reset the stop flag and spawn the capture thread
        self.capture_stop.store(false, Ordering::Relaxed);
        let handle = crate::audio::coreaudio_capture::spawn_capture_thread(
            self.analysis.clone(),
            self.capture_stop.clone(),
        );
        self.capture_handle = Some(handle);
    }

    #[cfg(target_os = "linux")]
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

        self.analysis.write(crate::audio::pipe::AudioAnalysis::new());
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn stop_capture(&mut self) {
        // Signal the capture thread to stop
        self.capture_stop.store(true, Ordering::Relaxed);

        if let Some(handle) = self.capture_handle.take() {
            // Give the thread a moment to exit cleanly
            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }
                if start.elapsed() > std::time::Duration::from_millis(300) {
                    break; // Don't block shutdown
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        self.analysis.write(crate::audio::pipe::AudioAnalysis::new());
    }

    /// Stop audio capture if it's currently running (called when visualizer is disabled)
    pub fn stop_capture_if_running(&mut self) {
        if self.has_real_audio() {
            self.stop_capture();
        }
    }

    /// Restart audio capture (called when visualizer is re-enabled while playing)
    pub fn restart_capture(&mut self) {
        if self.has_real_audio() || self.process.is_none() {
            return;
        }
        #[cfg(target_os = "linux")]
        if self.has_parec {
            self.start_capture();
        }
        #[cfg(target_os = "macos")]
        if self.has_coreaudio_tap {
            self.start_capture();
        }
        #[cfg(windows)]
        self.start_capture();
    }

    /// Kick off a background connect to the socket mpv creates for
    /// --input-ipc-server. This used to connect synchronously on the
    /// caller's thread with a blocking retry loop — harmless on Unix,
    /// where mpv typically creates the socket in well under 100ms, but
    /// the exact same pattern on Windows (see connect_pipe()) could
    /// freeze the whole app for up to 1.5s on every station change, and
    /// worse if mpv took longer than that to create the pipe. Since
    /// play_url() runs on the same thread as input handling and
    /// rendering (see main.rs's event loop), *any* blocking call here
    /// is a UI freeze — so connecting now happens entirely off that
    /// thread, and poll() picks up the result non-blockingly once ready.
    #[cfg(unix)]
    fn connect_ipc(&mut self) {
        let generation = self.ipc_generation;
        let socket_path = self.socket_path.clone();
        let (tx, rx) = mpsc::channel();

        // Retry for longer than the old synchronous version could afford
        // to — up to 100 * 50ms = 5s — since a slow-to-start mpv no
        // longer costs the user a frozen UI while we wait.
        std::thread::spawn(move || {
            for _ in 0..100 {
                match UnixStream::connect(&socket_path) {
                    Ok(stream) => {
                        stream.set_nonblocking(true).ok();
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_millis(5)))
                            .ok();
                        if let Some(reader) = stream.try_clone().ok().map(BufReader::new) {
                            let _ = tx.send((generation, stream, reader));
                        }
                        return;
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
        });

        self.ipc_connect_rx = Some(rx);
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

    /// Kick off a background connect to the named pipe mpv creates for
    /// --input-ipc-server. mpv creates the pipe server asynchronously
    /// after startup, and named-pipe creation on Windows can apparently
    /// take noticeably longer — and less reliably — than Unix socket
    /// creation in the wild. This used to retry synchronously on the
    /// caller's thread (play_url(), which runs on the same thread as
    /// input handling and rendering — see main.rs's event loop), so a
    /// slow-to-appear pipe froze the entire app for up to 1.5s on every
    /// station change, and left volume control permanently broken for
    /// that session if it took longer than that. Connecting now happens
    /// entirely off that thread; poll() picks up the result non-blockingly
    /// once ready.
    #[cfg(windows)]
    fn connect_pipe(&mut self) {
        use std::fs::OpenOptions;

        let generation = self.ipc_generation;
        let pipe_name = self.pipe_name.clone();
        let (tx, rx) = mpsc::channel();

        // Retry for longer than the old synchronous version could afford
        // to — up to 100 * 50ms = 5s — since a slow-to-appear pipe no
        // longer costs the user a frozen UI while we wait.
        std::thread::spawn(move || {
            for _ in 0..100 {
                match OpenOptions::new().read(true).write(true).open(&pipe_name) {
                    Ok(file) => {
                        // Named pipes have no non-blocking read without
                        // overlapped I/O, so a background thread does a
                        // blocking read_line() loop over its own cloned
                        // handle and forwards completed lines through a
                        // channel; poll() drains that channel non-blockingly,
                        // same shape as the Unix socket path.
                        if let Ok(read_handle) = file.try_clone() {
                            let (line_tx, line_rx) = mpsc::channel();
                            let reader_handle = std::thread::spawn(move || {
                                let mut reader = BufReader::new(read_handle);
                                let mut line = String::new();
                                loop {
                                    line.clear();
                                    match reader.read_line(&mut line) {
                                        Ok(0) => break, // EOF — mpv closed the pipe
                                        Ok(_) => {
                                            if line_tx.send(line.trim_end().to_string()).is_err() {
                                                break; // receiver dropped
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                            let _ = tx.send((generation, file, reader_handle, line_rx));
                        }
                        return;
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
        });

        self.ipc_connect_rx = Some(rx);
    }

    #[cfg(windows)]
    fn send_command(&mut self, command: &str) {
        if let Some(ref mut pipe) = self.pipe {
            let msg = format!("{}\n", command);
            if pipe.write_all(msg.as_bytes()).is_err() {
                self.pipe = None;
            }
        }
    }

    /// Runs once per mpv session when the IPC connection comes up.
    fn on_ipc_connected(&mut self) {
        self.send_command(
            "{ \"command\": [\"observe_property\", 1, \"media-title\"] }\n\
             { \"command\": [\"observe_property\", 2, \"audio-codec-name\"] }\n\
             { \"command\": [\"observe_property\", 3, \"audio-params/samplerate\"] }\n\
             { \"command\": [\"observe_property\", 4, \"audio-params/channel-count\"] }",
        );
        if self.playlist_mode {
            self.send_command(OBSERVE_PLAYLIST);
            for command in std::mem::take(&mut self.pending_commands) {
                self.send_command(&command);
            }
        }
    }

    fn ipc_connected(&self) -> bool {
        #[cfg(unix)]
        return self.stream.is_some();
        #[cfg(windows)]
        return self.pipe.is_some();
        #[cfg(not(any(unix, windows)))]
        return false;
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

        #[cfg(any(unix, windows))]
        {
            // Pick up a background connect_ipc()/connect_pipe() result if
            // one has arrived. If the generation doesn't match, a newer
            // play_url() call has already superseded this attempt (e.g.
            // the user switched stations again before it finished) —
            // discard it rather than wiring a dead connection into a
            // live session.
            #[cfg(unix)]
            {
                if let Some(rx) = self.ipc_connect_rx.take() {
                    match rx.try_recv() {
                        Ok((generation, stream, reader)) => {
                            if generation == self.ipc_generation {
                                self.stream = Some(stream);
                                self.reader = Some(reader);
                                self.on_ipc_connected();
                            }
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            self.ipc_connect_rx = Some(rx);
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            // Connector thread gave up — mpv never created
                            // the socket within the retry budget.
                        }
                    }
                }
            }

            #[cfg(windows)]
            {
                if let Some(rx) = self.ipc_connect_rx.take() {
                    match rx.try_recv() {
                        Ok((generation, file, reader_handle, line_rx)) => {
                            if generation == self.ipc_generation {
                                self.pipe = Some(file);
                                self.pipe_reader_handle = Some(reader_handle);
                                self.pipe_rx = Some(line_rx);
                                self.on_ipc_connected();
                            }
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            self.ipc_connect_rx = Some(rx);
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            // Connector thread gave up — mpv never created
                            // the pipe within the retry budget.
                        }
                    }
                }
            }

            // In fallback mode (no real capture backend), poll audio-pts for activity detection
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
                // Polled rather than observed: time-pos changes every frame
                if self.playlist_mode {
                    let request = json!({
                        "command": ["get_property", "time-pos"],
                        "request_id": mpv_ipc::TIME_POS_REQUEST_ID,
                    });
                    self.send_command(&request.to_string());
                }
            }

            #[cfg(unix)]
            if self.reader.is_none() {
                return;
            }
            #[cfg(windows)]
            if self.pipe_rx.is_none() {
                return;
            }

            let mut new_title: Option<String> = None;
            let mut got_audio_pts = false;
            let mut stream_closed = false;
            let mut completed_lines: Vec<String> = Vec::new();

            // On Unix, read directly off the non-blocking socket reader.
            // read_line() appends to self.line_buf rather than a fresh
            // local, and we only clear it once a full line (ending in
            // '\n') has actually arrived. If we hit WouldBlock partway
            // through a line, the bytes read so far stay in
            // self.line_buf and get completed on a later tick instead
            // of being thrown away.
            #[cfg(unix)]
            {
                let reader = self.reader.as_mut().unwrap();
                loop {
                    match reader.read_line(&mut self.line_buf) {
                        Ok(0) => {
                            // EOF — mpv closed the socket
                            stream_closed = true;
                            break;
                        }
                        Ok(_) => {
                            if self.line_buf.ends_with('\n') {
                                completed_lines.push(self.line_buf.trim_end().to_string());
                                self.line_buf.clear();
                            } else {
                                // Socket closed mid-line with no trailing
                                // newline — nothing more is coming.
                                stream_closed = true;
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => {
                            stream_closed = true;
                            break;
                        }
                    }
                }
            }

            // On Windows, a background thread already did the blocking
            // read_line() work (see connect_pipe()) — here we just drain
            // the completed lines it's produced so far.
            #[cfg(windows)]
            {
                while let Ok(line) = self.pipe_rx.as_ref().unwrap().try_recv() {
                    completed_lines.push(line);
                }
                // If the reader thread has exited, mpv closed the pipe
                // (or the read errored) and nothing more is coming.
                if self
                    .pipe_reader_handle
                    .as_ref()
                    .map(|h| h.is_finished())
                    .unwrap_or(false)
                {
                    stream_closed = true;
                }
            }

            for text in &completed_lines {
                if let Some(title) = mpv_ipc::extract_media_title(text) {
                    if !title.is_empty() {
                        new_title = Some(title);
                    }
                }

                if text.contains("\"request_id\":100") || text.contains("\"request_id\": 100") {
                    if text.contains("\"data\":") && !text.contains("\"error\"") {
                        got_audio_pts = true;
                    }
                }

                mpv_ipc::parse_stream_info(&mut self.stream_info, text);
                if self.playlist_mode {
                    mpv_ipc::parse_playback_state(&mut self.playback, text);
                }
            }

            if stream_closed {
                #[cfg(unix)]
                {
                    self.stream = None;
                    self.reader = None;
                    self.line_buf.clear();
                }
                #[cfg(windows)]
                {
                    self.pipe = None;
                    self.pipe_rx = None;
                    if let Some(handle) = self.pipe_reader_handle.take() {
                        let _ = handle.join();
                    }
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

    pub fn stop(&mut self) {
        // Bump first: any connect_ipc()/connect_pipe() background attempt
        // still in flight is tagged with the old generation, so poll()
        // will recognize and discard its result as stale even if it
        // arrives after this call returns.
        self.ipc_generation = self.ipc_generation.wrapping_add(1);

        #[cfg(unix)]
        {
            self.stream = None;
            self.reader = None;
            self.line_buf.clear();
            self.ipc_connect_rx = None;
        }

        #[cfg(windows)]
        {
            self.pipe = None;
            self.ipc_connect_rx = None;
        }

        // Stop audio capture first
        self.stop_capture();

        // Then stop mpv. On Windows this must happen before we join the
        // pipe reader thread below: that thread blocks on a synchronous
        // read until mpv closes its end of the pipe (EOF), so joining it
        // first would hang.
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        #[cfg(windows)]
        {
            self.pipe_rx = None;
            if let Some(handle) = self.pipe_reader_handle.take() {
                let start = std::time::Instant::now();
                loop {
                    if handle.is_finished() {
                        let _ = handle.join();
                        break;
                    }
                    if start.elapsed() > std::time::Duration::from_millis(300) {
                        break; // Don't block shutdown
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }

        let _ = std::fs::remove_file(&self.socket_path);
        self.audio_level = 0.0;
        self.media_title = None;
        self.stream_info.reset();
        self.playlist_mode = false;
        self.pending_commands.clear();
        self.playback = PlaybackState::default();
    }

    pub fn is_playing(&self) -> bool {
        self.process.is_some()
    }
}

// ── Playlist sessions ─────────────────────────────────────────────────
// Used for on-demand music (Subsonic), where mpv plays a queue of tracks
// and advances through it itself. None of this runs for radio streams.
impl Player {
    /// Starts mpv idle and loads `urls` as its playlist, beginning at
    /// `start`. The URLs go over IPC rather than mpv's command line so
    /// they — and any auth tokens in them — don't appear in `ps` output.
    pub fn play_playlist(&mut self, urls: &[String], start: usize, volume: u32) -> bool {
        self.stop();
        if start >= urls.len() {
            return false;
        }

        self.playlist_mode = true;
        for url in urls {
            self.pending_commands.push(mpv_ipc::command(&[json!("loadfile"), json!(url), json!("append")]));
        }
        self.pending_commands.push(mpv_ipc::command(&[json!("playlist-play-index"), json!(start)]));

        if self.spawn_mpv(None, volume) {
            true
        } else {
            self.playlist_mode = false;
            self.pending_commands.clear();
            false
        }
    }

    pub fn is_playlist(&self) -> bool {
        self.playlist_mode
    }

    pub fn toggle_pause(&mut self) {
        self.playlist_command(&[json!("cycle"), json!("pause")]);
    }

    pub fn playlist_next(&mut self) {
        self.playlist_command(&[json!("playlist-next")]);
    }

    pub fn playlist_prev(&mut self) {
        self.playlist_command(&[json!("playlist-prev")]);
    }

    pub fn play_index(&mut self, index: usize) {
        self.playlist_command(&[json!("playlist-play-index"), json!(index)]);
    }

    /// Seek by `seconds` (negative = back) within the current track.
    pub fn seek_relative(&mut self, seconds: f64) {
        self.playlist_command(&[json!("seek"), json!(seconds), json!("relative")]);
    }

    pub fn seek_to(&mut self, seconds: f64) {
        self.playlist_command(&[json!("seek"), json!(seconds), json!("absolute")]);
    }

    /// Adds a track to the end of the playlist without interrupting playback.
    pub fn append(&mut self, url: &str) {
        self.playlist_command(&[json!("loadfile"), json!(url), json!("append")]);
    }

    /// Sends now if connected, otherwise queues for on_ipc_connected().
    /// No-op outside playlist sessions.
    fn playlist_command(&mut self, args: &[serde_json::Value]) {
        if !self.playlist_mode {
            return;
        }
        let command = mpv_ipc::command(args);
        if self.ipc_connected() {
            self.send_command(&command);
        } else {
            self.pending_commands.push(command);
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "linux")]
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}