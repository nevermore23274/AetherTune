# Profiling & Performance Tuning

AetherTune includes a built-in profiler that measures per-frame timing without external tools. This guide explains how to read the profiler, understand the numbers, and optimize performance for your system.

## Opening the Profiler

Press `` ` `` (backtick) to toggle the profiler overlay. While it's open:

- `>` or `.` — decrease tick rate by 10ms (faster updates, more CPU)
- `<` or `,` — increase tick rate by 10ms (slower updates, less CPU)

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

### The avg and max Columns

**avg** is the mean over the rolling window. **max** is the highest value seen in that same window. Both use a 2-second rolling window (~60 frames), so old spikes fall off naturally — you're always seeing recent performance, not a startup spike from minutes ago.

Max values are color-coded: green under 5,000µs, yellow under 10,000µs, red above.

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

## Reference Benchmarks

Measured on a Ryzen 9 5900X running Hyprland/PipeWire, kitty terminal, streaming 192kbps MP3 with real audio visualization active:

| Tick rate | FPS | Avg draw | Avg work | CPU load | Status |
|-----------|-----|----------|----------|----------|--------|
| 10ms | 100 | ~5,500µs | ~5,550µs | 55% | OK |
| 20ms | 50 | ~5,500µs | ~5,550µs | 27% | IDLE |
| 30ms | 33 | ~5,500µs | ~5,550µs | 18% | IDLE |
| 50ms | 20 | ~5,500µs | ~5,550µs | 11% | IDLE |

Key observations:

- **Draw cost is constant** (~5.5ms) regardless of tick rate. It dominates the work budget.
- **IPC poll and visualizer** are negligible (20–80µs combined on tick frames).
- **Load scales inversely with tick rate** since the same work runs against a larger budget.
- **30ms (33 FPS)** is the default — good balance of visualizer smoothness vs resource usage.

## Testing

### Current Coverage

AetherTune has unit tests across the `audio::pipe` and `audio::visualizer` modules, covering FFT computation, frequency band analysis, visualizer state management, and gravity/smoothing constants.

```bash
# Run all tests
cargo test

# Run with output visible
cargo test -- --nocapture

# Run a specific test module
cargo test audio::pipe
```

### CI

GitHub Actions runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on pushes to `main` and `dev`. Release builds are triggered by version tags.