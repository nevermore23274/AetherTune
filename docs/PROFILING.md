# Profiling & Performance Tuning

AetherTune includes a built-in profiler that measures per-frame timing without external tools. This guide explains how to read the profiler, understand the numbers, and optimize performance for your system.

## Opening the Profiler

Press `` ` `` (backtick) to toggle the profiler overlay. While it's open:

- `>` or `.` — decrease tick rate by 10ms (faster updates, more CPU)
- `<` or `,` — increase tick rate by 10ms (slower updates, less CPU)
- `}` — increase smoothing by 5% (smoother bars, more visual lag)
- `{` — decrease smoothing by 5% (snappier bars, more jitter)

## Reading the Profiler

### Load and Status

The top of the profiler shows:

```
Tick 30ms  │  ~33 FPS  │  < > adjust
Load 23%  (7050 / 30000µs)  OK
```

**Tick** is your current tick rate — how often the app updates. Lower means smoother but more CPU.

**Load** is the percentage of your tick budget consumed by actual CPU work — **not** your system CPU usage. The budget is simply your tick rate in microseconds (30ms = 30,000µs). If your CPU work takes 7,050µs, that's 23% of the budget. On a modern CPU, AetherTune typically uses 1–3% of total system CPU; the "load" percentage here tells you how close the app is to running out of time within each frame.

**Status labels** tell you at a glance if your system is keeping up:

| Status | Load | Meaning |
|--------|------|---------|
| **IDLE** | <30% | Plenty of headroom, system is barely working |
| **OK** | 30–60% | Healthy range with room for spikes |
| **TIGHT** | 60–80% | Getting close to the limit, spikes may cause stutter |
| **OVER BUDGET** | >80% | The app is struggling to keep up — increase tick rate |

### Sparkline

The bar below the load shows CPU load over the last ~40 frames. This lets you see spikes and trends at a glance rather than watching numbers change. The color matches the current status.

### Per-frame Work (all frames)

These timings are averaged across every frame:

| Metric | What it measures |
|--------|-----------------|
| **Draw** | `terminal.draw()` — rendering all UI panels and flushing to the terminal |
| **Key input** | Processing a key event after `event::poll()` returns |

Draw is almost always the dominant cost. It's the time ratatui spends computing the layout, building styled spans, diffing against the previous frame, and writing escape sequences to stdout.

### Tick Work (tick frames only)

These are averaged only over frames where the tick actually ran. On most frames, the tick doesn't fire (the app is just drawing and waiting for events), so averaging these across all frames would make them look artificially close to zero.

| Metric | What it measures |
|--------|-----------------|
| **IPC poll** | `player.poll()` — reading the mpv IPC socket for media title and stream info |
| **Visualizer** | `visualizer.tick_real()` or `tick_simulated()` — reading audio analysis and updating bar heights |

These should be well under 100µs each. If IPC poll is consistently high, it may indicate the mpv socket is backed up with responses.

### Totals

| Metric | What it measures |
|--------|-----------------|
| **CPU work** | Sum of draw + key input + IPC poll + visualizer |
| **Idle wait** | Time spent sleeping in `event::poll()`, waiting for input or timeout. This is not CPU work. |
| **Frame** | Full wall-clock time per loop iteration (work + idle) |

Idle wait and Frame use **inverted color coding**: high values are green (the CPU is mostly sleeping, which is good), low values are red (the app is struggling to find idle time).

### Visualizer Responsiveness

This section shows how quickly the visualizer bars react to changes in the audio signal.

```
Smoothing     70%  │  { } adjust
Settling      7 frames  (~70ms @ 10ms tick)
FFT rate      96/s  (~10.5ms per update)
```

**Smoothing** is the noise reduction weight used in the visualizer's integral smoothing (an exponential moving average). Higher values produce smoother, more flowing bar animation but introduce visual lag. Lower values make bars snap to the audio faster but may look jittery. The default is 70% (CAVA uses 77%). Adjustable in 5% steps with `{` and `}`.

**Settling** is the theoretical number of frames for bars to reach 90% of a new target value, calculated as `⌈ln(0.1) / ln(smoothing)⌉`. The millisecond estimate multiplies this by your current tick rate. Color coding: green under 200ms, yellow under 500ms, red above.

| Smoothing | Settling (frames) | @ 10ms tick | @ 30ms tick | Feel |
|-----------|-------------------|-------------|-------------|------|
| 50% | 4 | 40ms | 120ms | Snappy, some jitter |
| 70% | 7 | 70ms | 210ms | Default — responsive with smooth flow |
| 85% | 15 | 150ms | 450ms | Very smooth, noticeable lag |
| 95% | 45 | 450ms | 1350ms | Flowing but sluggish |

The smoothing value is runtime-only and resets to the default (70%) on restart.

**FFT rate** shows how many FFT updates per second the audio reader thread is producing. The reader uses a sliding window — it reads 512 new samples (~10.7ms of audio at 48kHz) at a time, shifts the 1024-sample buffer, and runs a full FFT on the window. This produces ~94 updates/sec, roughly 2× what a full 1024-sample read would give (~47/s), without sacrificing frequency resolution. Color coding: green ≥80/s, yellow ≥40/s, red below. Shows "—" when no audio capture is active (e.g., on macOS or when parec is unavailable on Linux).

### The avg and max Columns

**avg** is the mean over the rolling window. **max** is the highest value seen in that same window. Both use a 2-second rolling window (~60 frames), so old spikes fall off naturally — you're always seeing recent performance, not a startup spike from minutes ago.

For CPU work metrics (Draw, Key input, IPC poll, Visualizer, CPU work), values are color-coded where low is green and high is red. For idle metrics (Idle wait, Frame), the coloring is inverted — high values are green because they indicate the CPU is mostly sleeping.

## Optimizing for Your System

### Step 1: Check the status label

If it says **IDLE** or **OK**, you're fine. No changes needed.

### Step 2: If it says TIGHT or OVER BUDGET

Open the profiler and press `<` a few times to increase the tick rate. Going from 30ms to 50ms cuts CPU usage significantly while still giving a smooth visualizer. The tradeoff is slightly less responsive bar animation.

| Tick rate | FPS | Visualizer feel |
|-----------|-----|-----------------|
| 10ms | 100 | Silky smooth, high CPU |
| 20ms | 50 | Very responsive |
| 30ms | 33 | Default — good balance |
| 50ms | 20 | Still smooth, much lighter |
| 80ms | 12 | Noticeable stepping, very light |

### Step 3: If Draw is the bottleneck

Draw cost is almost always the dominant factor. It scales with terminal size (more cells = more work) and the number of UI elements visible. Things that increase draw cost:

- Large terminal windows (4K monitors at small font sizes)
- Many stations loaded in the list
- Long song log history
- Running over SSH (network latency added to each flush)

If draw is consistently over 10,000µs, consider increasing the tick rate or reducing your terminal size.

### Step 4: Profiling over SSH

Over SSH, draw cost increases dramatically because each `terminal.draw()` flushes escape sequences over the network. If you're seeing 20,000µs+ draw times over SSH, increase the tick rate to 80ms or higher.

## Implementation Details

Timing uses `std::time::Instant` with microsecond precision. Samples are stored in a ring buffer of 120 entries. The profiler overlay itself adds minimal overhead (one extra ratatui paragraph render per frame).

Each frame records a `had_tick` flag indicating whether the IPC poll and visualizer actually ran that iteration. The `summary()` method uses this to compute tick-only averages separately from per-frame averages, giving accurate numbers for both.

The sparkline records one CPU load sample per frame into a separate 40-entry ring buffer, displayed oldest-to-newest.

### Station List Rendering

The Stations and History panels mark favorited entries with a ★. This lookup builds a `HashSet` of favorite URLs once per `draw()` call rather than linear-scanning `app.favorites.entries` for every row with N stations loaded and M favorites, that's O(N) hash lookups per frame instead of O(N × M) string comparisons. This matters most after using "Load more" repeatedly, since the station list (unlike the always-small favorites/history lists) can grow into the hundreds over a session. This is part of what "Many stations loaded in the list" refers to under Draw cost above.

### Audio Reader (Sliding Window)

The audio reader thread (Unix `parec` FIFO reader or Windows WASAPI loopback) uses a sliding window to maximize FFT update rate without sacrificing frequency resolution. Instead of reading a full 1024-sample chunk (~21ms at 48kHz) before running FFT, it reads 512 new samples (~10.7ms), shifts the 1024-sample buffer left, and runs FFT on the full window. This produces ~94 FFT updates/sec at 2× the rate of full-chunk reads while keeping the same 1024-point FFT resolution. The `fft_count` field on `AudioAnalysis` is incremented on each update, and the profiler computes the rate by sampling this counter every ~1 second.

#### Considered: sliding DFT instead of full FFT recompute

Since only half the 1024-sample window actually changes between steps, it's tempting to think a "true" incremental sliding DFT. Updating each frequency bin sample-by-sample via the standard recurrence `X_k[n] = (X_k[n-1] - x[n-N] + x[n]) * e^{j*2*pi*k/N}` which would be cheaper than recomputing the full FFT from scratch every step.

Benchmarked with the actual FFT size (1024) and step size (512 new samples/update): the sliding DFT was **~16.5x slower** (293µs vs 18µs per update). The reason is fundamental, not an implementation issue: the sliding DFT's per-sample update is O(N) across all bins, and its efficiency advantage only exists when a fresh spectrum is needed on *every single incoming sample*. Here, a spectrum is only needed once per 512-sample step, so 512 individual O(N) updates (≈524K operations) cost far more than one O(N log N) FFT (≈10K operations). The current full-recompute-per-step design is the correct choice for this access pattern and not an oversight. This is left here so it isn't reinvestigated as a "missed" optimization later.

## Reference Benchmarks

Measured on a Ryzen 9 5900X running Hyprland/PipeWire, kitty terminal, streaming 192kbps MP3 with real audio visualization active:

| Tick rate | FPS | Avg draw | Avg work | CPU load | FFT rate | Status |
|-----------|-----|----------|----------|----------|----------|--------|
| 10ms | 100 | ~5,700µs | ~5,770µs | 57% | ~96/s | OK |
| 20ms | 50 | ~5,500µs | ~5,550µs | 27% | ~96/s | IDLE |
| 30ms | 33 | ~5,500µs | ~5,550µs | 18% | ~96/s | IDLE |
| 50ms | 20 | ~5,500µs | ~5,550µs | 11% | ~96/s | IDLE |

Key observations:

- **Draw cost is constant** (~5.5–5.7ms) regardless of tick rate. It dominates the work budget.
- **IPC poll and visualizer** are negligible (20–80µs combined on tick frames).
- **FFT rate is constant** (~96/s) regardless of tick rate — the reader thread runs independently.
- **Load scales inversely with tick rate** since the same work runs against a larger budget.
- **30ms (33 FPS)** is the default — good balance of visualizer smoothness vs resource usage.

## Testing

### Current Coverage

AetherTune has unit tests across the `audio::fft`, `audio::visualizer`, and `storage::config` modules, covering FFT computation, frequency band analysis, visualizer state management, and gravity/smoothing constants.

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture

# Run a specific test module
cargo test audio::fft
```

### CI

GitHub Actions runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on pushes to `main` and `dev`. Release builds are triggered by version tags.