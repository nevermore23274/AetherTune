# AetherTune

A terminal-based internet radio player with real-time audio visualization, built in Rust.

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
[![Release](https://github.com/nevermore23274/AetherTune/actions/workflows/release.yml/badge.svg)](https://github.com/nevermore23274/AetherTune/actions/workflows/release.yml)
[![AUR](https://img.shields.io/aur/version/aethertune-bin)](https://aur.archlinux.org/packages/aethertune-bin)

## Overview

AetherTune is a TUI (terminal user interface) application that lets you browse, search, and stream internet radio stations directly from your terminal. It features a real-time spectrum visualizer driven by actual audio analysis, a rolling song log that captures ICY metadata, and live stream health monitoring.

![AetherTune](img/screenshot.png)

## Requirements

- **Rust** 1.85+ (edition 2024)
- **mpv** — audio playback backend
- **PulseAudio or PipeWire** (with PulseAudio compatibility) — for real audio visualization
  - `parec` must be available in `$PATH` (provided by `pulseaudio-utils` or `pipewire-pulse`)
- **Linux** — uses Unix domain sockets for mpv IPC, POSIX FIFOs for audio capture, and `libc` for process group management

### System dependencies (Debian/Ubuntu)

```bash
sudo apt install mpv pulseaudio-utils
```

### System dependencies (Arch)

```bash
sudo pacman -S mpv pipewire-pulse
# or: sudo pacman -S mpv pulseaudio
```

### Features

- **Station browsing** — browse thousands of stations via the RadioBrowser API, filter by genre, search by name
- **Real-time audio visualization** — 16-band spectrum analyzer using DFT on captured PCM audio via PulseAudio/PipeWire monitor, with CAVA-inspired gravity fall-off, integral smoothing, and automatic sensitivity
- **Song log** — automatically tracks song changes from ICY stream metadata with timestamps
- **Stream health monitor** — live bitrate (actual vs advertised), buffer status, codec info, connection uptime
- **Favorites & history** — save stations, track listening history, persisted to JSON
- **Built-in profiler** — per-frame timing breakdown for performance tuning
- **Fallback mode** — simulated visualizer when PulseAudio capture isn't available

### Optional

- Without `parec`, the app falls back to a simulated visualizer — everything else works normally.

## Installation

### Arch Linux (AUR)

```bash
paru -S aethertune-bin
```

Or with yay: `yay -S aethertune-bin`

### From source

```bash
git clone https://github.com/nevermore23274/aethertune.git
cd aethertune
cargo build --release
./target/release/AetherTune
```

### Prebuilt binary

Download the latest release from the [Releases page](https://github.com/nevermore23274/AetherTune/releases):

```bash
# Download and extract (replace VERSION with the actual tag, e.g. v0.2.0)
curl -LO https://github.com/nevermore23274/AetherTune/releases/download/VERSION/AetherTune-VERSION-linux-x86_64.tar.gz
tar xzf AetherTune-VERSION-linux-x86_64.tar.gz
./AetherTune-VERSION-linux-x86_64/AetherTune
```

> **Note:** You still need `mpv` and `parec` installed on your system regardless of install method. AUR users get `mpv` automatically as a dependency.

## Usage

```bash
# Run normally (with CRT boot animation)
aethertune

# Skip the launch menu
aethertune --skip-menu

# Adjust boot animation speed (fast, normal, slow, off)
aethertune --boot-speed=fast
```

> If installed from source, use `./target/release/AetherTune` instead of `aethertune`.

## Keybindings

Below is a list of keyboard shortcuts. Press `?` in the app to see them as well (`Esc` closes the overlay).

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate station list |
| `Enter` | Play selected station |
| `s` | Stop playback |
| `+` / `-` | Volume up / down |
| `/` | Search stations |
| `f` | Toggle favorite |
| `i` | Station details overlay |
| `n` | Load more stations |
| `Tab` | Cycle panel (Stations / Favorites / History) |
| `[` / `]` | Cycle genre category |
| `Shift+Tab` | Cycle genre category (backward) |
| `?` | Help overlay |
| `` ` `` | Performance profiler |
| `<` / `>` | Adjust tick rate (when profiler is open) |
| `q` | Quit |

## Architecture

```
src/
├── main.rs                  Entry point, event loop, frame timing
├── app.rs                   App state, business logic, perf stats
├── audio/
│   ├── player.rs            mpv playback, IPC, parec capture, stream info
│   ├── pipe.rs              FIFO creation, PCM reader thread, DFT analysis
│   └── visualizer.rs        Bar animation (real + simulated modes)
├── storage/
│   ├── config.rs            User preferences (tick rate, volume)
│   ├── favorites.rs         JSON persistence for favorites
│   └── history.rs           JSON persistence for play history
└── ui/
    ├── mod.rs               Layout orchestration
    ├── helpers.rs            Color palette, shared widgets
    ├── launcher.rs           CRT boot animation + start menu
    ├── header.rs             Top bar (LIVE indicator, genre, hints)
    ├── station_list.rs       Left panel (stations/favorites/history)
    ├── now_playing.rs        Station info + session timer
    ├── song_log.rs           Rolling ICY metadata log
    ├── visualizer.rs         Spectrum bar rendering
    ├── stream_info.rs        Live stream health panel
    ├── media_browser.rs      Media source switcher (Radio/Subsonic stub)
    ├── overlays.rs           Help + station detail popups
    └── perf_overlay.rs       Built-in performance profiler
```

### Audio visualization pipeline

When `parec` is available, AetherTune captures audio through the PulseAudio/PipeWire monitor source:

1. **mpv** plays audio normally through the default audio output
2. **parec** captures the monitor source and writes raw s16le stereo 48kHz PCM to a named FIFO
3. A background thread reads the FIFO and runs a **partial DFT with Hann windowing** across 16 logarithmically-spaced frequency bands (50Hz–10kHz)
4. Band energies and RMS are pushed to a shared `Arc<Mutex<AudioAnalysis>>`
5. The visualizer applies CAVA-inspired post-processing: gravity fall-off (accelerating drop), integral smoothing (weighted running average), and automatic sensitivity adjustment

Process isolation is handled carefully: `parec` runs in its own process group via `setsid()`, and cleanup uses `kill(-pgid, SIGTERM)` to ensure no orphaned processes.

### Data persistence

Favorites, history, and user preferences (tick rate, volume) are stored as JSON in `~/.aethertune/`. The serializer/parser is hand-rolled (no serde dependency) to keep the dependency tree minimal. Settings like tick rate are saved automatically when adjusted and restored on next launch.

## License

MIT