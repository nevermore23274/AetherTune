# Profiling & Testing

Documentation for AetherTune's built-in profiler and plans for test coverage.

## Built-in Profiler

AetherTune includes a lightweight self-profiler that measures per-frame timing without external tools. It runs inside the TUI itself and imposes negligible overhead.

### Usage

Press `` ` `` (backtick) to toggle the profiler overlay. While the overlay is open:

- `>` or `.` — decrease tick rate by 10ms (faster updates, more CPU)
- `<` or `,` — increase tick rate by 10ms (slower updates, less CPU)

### What it measures

Each frame of the main loop is broken into four timed phases:

| Phase | What it measures |
|-------|-----------------|
| **Draw** | `terminal.draw()` — rendering all UI panels to the terminal buffer and flushing |
| **Key handle** | Processing a key event after `event::poll()` returns (excludes idle wait time) |
| **IPC poll** | `player.poll()` — reading mpv IPC socket for media title, stream info properties |
| **Visualizer** | `visualizer.tick_real()` or `tick_simulated()` — reading audio analysis and updating bar heights |

The profiler also shows:

- **Event wait** (idle) — time spent sleeping in `crossterm::event::poll()`, waiting for input or timeout. This is *not* CPU work.
- **CPU load** — `work_total / tick_budget × 100%`. Only counts actual CPU work, not idle wait.
- **Frame** (wall total) — full wall-clock time per loop iteration.

### Implementation details

Timing uses `std::time::Instant` with microsecond precision. Samples are stored in a ring buffer of 120 entries (~4 seconds at 30ms tick rate). The overlay shows rolling averages and per-field maximums.

The key insight that led to this design: the original profiler showed 100% budget usage because `event::poll()` sleeps for the remaining tick budget. Splitting the event phase into "wait" (idle sleep) and "handle" (actual key processing) revealed that real CPU work is only ~7ms per frame — dominated entirely by terminal rendering.

### Reference benchmarks

Measured on a Ryzen 9 5900X running Hyprland/PipeWire, streaming 192kbps MP3 with real audio visualization active:

| Tick rate | FPS | Avg work | CPU load | System CPU | Memory |
|-----------|-----|----------|----------|------------|--------|
| 100ms | 10 | ~6,860µs | 6% | <1% | 24MB |
| 80ms | 12 | ~6,660µs | 8% | ~1% | 24MB |
| 30ms | 33 | ~7,050µs | 23% | ~1.6% | 24MB |
| 20ms | 50 | ~7,150µs | 35% | ~2% | 24MB |

Key observations:

- **Draw cost is constant** (~7ms) regardless of tick rate. It dominates the work budget.
- **IPC poll and visualizer** are negligible (25–40µs combined).
- **Work total scales linearly** with FPS since each frame does the same amount of work.
- **System CPU stays very low** even at 50 FPS due to the efficient event-driven architecture.
- **30ms (33 FPS)** was chosen as the default — best balance of visualizer responsiveness vs resource usage.

### Optimizing draw cost

If draw cost becomes a concern (e.g., on slower hardware or over SSH), potential optimizations:

- Reduce visualizer bar count (currently 24 bars × direct buffer writes)
- Use `ratatui`'s diff-based rendering (already enabled by default)
- Skip drawing unchanged panels when no state has changed
- Reduce the number of `Span` allocations in station list rendering

## Testing

### Current state

AetherTune currently has **no automated tests**. All validation has been done through manual testing and the built-in profiler. This section documents the testing plan.

### Unit test candidates

#### `audio::pipe` — DFT and frequency analysis

The DFT computation and band energy calculation are pure functions that are ideal for unit testing:

```rust
// Test that a known sine wave produces energy in the expected band
#[test]
fn test_dft_single_tone() {
    let sample_rate = 48000;
    let freq = 440.0; // A4
    let samples: Vec<i16> = (0..1024)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            (f64::sin(2.0 * std::f64::consts::PI * freq * t) * 16000.0) as i16
        })
        .collect();
    
    let energies = compute_band_energies(&samples);
    // Band covering 440Hz should have the highest energy
    // Bands far from 440Hz should be near zero
}

#[test]
fn test_dft_silence() {
    let samples = vec![0i16; 1024];
    let energies = compute_band_energies(&samples);
    assert!(energies.iter().all(|&e| e < 0.001));
}

#[test]
fn test_band_frequencies_are_logarithmic() {
    // Verify the 24 bands span 50Hz–18kHz with log spacing
}
```

To make these testable, `compute_band_energies` would need to be extracted as a public function (currently it operates on the shared buffer internally).

#### `audio::visualizer` — smoothing and peak decay

```rust
#[test]
fn test_peak_decay() {
    let mut vis = Visualizer::new();
    // Set bars high, then tick with zero input
    // Peaks should decay gradually, not jump to zero
}

#[test]
fn test_volume_scaling() {
    // At volume 0, bars should be zero
    // At volume 100, bars should reflect full input
}
```

#### `app` — song change detection and stream noise filtering

```rust
#[test]
fn test_is_stream_noise_urls() {
    assert!(App::is_stream_noise("http://67.249.184.45:8015/", &None));
    assert!(App::is_stream_noise("https://stream.example.com/live", &None));
}

#[test]
fn test_is_stream_noise_slugs() {
    assert!(App::is_stream_noise("highvoltage_mobile_mp3", &None));
    assert!(App::is_stream_noise("stream.mp3", &None));
}

#[test]
fn test_is_not_stream_noise() {
    assert!(!App::is_stream_noise("DOWN - Stone the Crow", &None));
    assert!(!App::is_stream_noise("Soundgarden - Fell on Black Days", &None));
    assert!(!App::is_stream_noise("Disturbed - Stupify", &None));
}
```

The `is_stream_noise` method is already a static function — it just needs to be made `pub` for testing.

#### `storage::favorites` and `storage::history` — JSON serialization

```rust
#[test]
fn test_favorites_roundtrip() {
    // Create a FavoritesStore, add entries, serialize, deserialize, compare
}

#[test]
fn test_history_deduplication() {
    // Adding the same station URL twice should update, not duplicate
}

#[test]
fn test_history_max_entries() {
    // Adding more than 50 entries should truncate oldest
}
```

#### `audio::player` — JSON parsing helpers

```rust
#[test]
fn test_extract_number() {
    assert_eq!(Player::extract_number(r#"{"data":128000.0}"#), Some(128000.0));
    assert_eq!(Player::extract_number(r#"{"data": 44100}"#), Some(44100.0));
}

#[test]
fn test_extract_string_value() {
    assert_eq!(
        Player::extract_string_value(r#"{"data":"mp3"}"#),
        Some("mp3".to_string())
    );
}

#[test]
fn test_extract_media_title() {
    let json = r#"{"event":"property-change","name":"media-title","data":"Artist - Song"}"#;
    assert_eq!(Player::extract_media_title(json), Some("Artist - Song".to_string()));
}
```

These parsers are already static methods — they just need `pub` visibility.

### Integration test candidates

These require more infrastructure but would catch real issues:

- **mpv IPC round-trip** — spawn mpv with a test stream URL, verify that `poll()` receives media title updates
- **FIFO pipeline** — create a FIFO, write known PCM data, verify that `spawn_reader` produces expected band energies
- **RadioBrowser API** — verify station search returns results (network-dependent, may want to mock)

### Running tests

Once tests are added:

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture

# Run a specific test module
cargo test audio::pipe

# Run only the song noise filter tests
cargo test is_stream_noise
```

### CI considerations

- Unit tests for pure functions (DFT, JSON parsing, noise filtering) can run anywhere
- Integration tests requiring mpv/parec should be gated behind a feature flag or CI environment variable
- The RadioBrowser API tests should be marked `#[ignore]` by default to avoid network flakiness in CI
