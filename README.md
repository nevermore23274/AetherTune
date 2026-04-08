# AetherTune

A terminal-based internet radio player with real-time audio visualization, built in Rust.

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
[![Release](https://github.com/nevermore23274/AetherTune/actions/workflows/release.yml/badge.svg)](https://github.com/nevermore23274/AetherTune/actions/workflows/release.yml)
[![AUR](https://img.shields.io/aur/version/aethertune-bin)](https://aur.archlinux.org/packages/aethertune-bin)
[![PPA](https://img.shields.io/badge/PPA-patchgoblin%2Faethertune-orange)](https://launchpad.net/~patchgoblin/+archive/ubuntu/aethertune)
[![Homebrew](https://img.shields.io/badge/brew-nevermore23274%2Faethertune-yellow)](https://github.com/nevermore23274/homebrew-aethertune)

## Overview

AetherTune is a TUI (terminal user interface) application that lets you browse, search, and stream internet radio stations directly from your terminal. It features a real-time spectrum visualizer driven by actual audio analysis, a rolling song log that captures ICY metadata, and live stream health monitoring.

[▶ Watch the demo](https://github.com/nevermore23274/AetherTune/raw/main/img/Showcase.mp4)

### Features

- **Station browsing** — browse thousands of stations via the RadioBrowser API, filter by genre, search by name. Results are sorted by popularity with broken streams and spam filtered out automatically
- **Local blending** — optionally configure your country code in Settings to blend ~30% local stations into every genre and search result, interleaved naturally with global results
- **Real-time audio visualization** — 16-band spectrum analyzer using an in-place radix-2 FFT on captured PCM audio via PulseAudio/PipeWire monitor, with CAVA-inspired gravity fall-off, integral smoothing, and automatic sensitivity
- **Song log** — automatically tracks song changes from ICY stream metadata with timestamps
- **Stream health monitor** — live bitrate (actual vs advertised), buffer status, codec info, connection uptime
- **Favorites & history** — save stations, track listening history, persisted to JSON
- **Built-in profiler** — per-frame timing breakdown for performance tuning
- **Fallback mode** — simulated visualizer when PulseAudio capture isn't available

### Optional

- Without `parec`, the app falls back to a simulated visualizer — everything else works normally.

## Installation

<details>
<summary><b>Linux — Arch (AUR)</b></summary>

```bash
paru -S aethertune-bin
```

Or with yay: `yay -S aethertune-bin`

Dependencies (`mpv`, `libpulse`) are installed automatically. For real-time audio visualization, you also need `pipewire-pulse` or `pulseaudio` (one is likely already installed).

</details>

<details>
<summary><b>Linux — Ubuntu/Debian (PPA)</b></summary>

```bash
sudo add-apt-repository ppa:patchgoblin/aethertune
sudo apt update
sudo apt install aethertune
```

Currently available for Ubuntu Noble (24.04). Dependencies (`mpv`, `libpulse0`) are installed automatically. For real-time audio visualization, you also need `pipewire-pulse` or `pulseaudio`.

</details>

<details>
<summary><b>Linux — Homebrew</b></summary>

```bash
brew tap nevermore23274/aethertune
brew install aethertune
```

You'll also need `mpv` and `pulseaudio-utils` (or `pipewire-pulse`) installed on your system for playback and real-time audio visualization.

</details>

<details>
<summary><b>Linux — Prebuilt binary</b></summary>

Download the latest `.tar.gz` from the [Releases page](https://github.com/nevermore23274/AetherTune/releases):

```bash
curl -LO https://github.com/nevermore23274/AetherTune/releases/download/VERSION/AetherTune-VERSION-linux-x86_64.tar.gz
tar xzf AetherTune-VERSION-linux-x86_64.tar.gz
./AetherTune-VERSION-linux-x86_64/AetherTune
```

Replace `VERSION` with the actual tag (e.g. `v0.5.1`). You'll need `mpv` and `parec` installed on your system.

</details>

<details>
<summary><b>Nix / Flakes</b></summary>

If you use Nix with flakes enabled:

```bash
# Run directly
nix run github:nevermore23274/AetherTune
```

You can also add AetherTune as a flake input in your own `flake.nix`:

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    AetherTune.url = "github:nevermore23274/AetherTune";
  };

  outputs =
    inputs@{
      self,
      flake-utils,
      nixpkgs,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [ inputs.AetherTune.packages.${system}.aethertune ];
        };
      }
    );
}
```

</details>

<details>
<summary><b>Linux — From source</b></summary>

Requires Rust 1.85+ and system dependencies (`mpv`, `pulseaudio-utils` or `pipewire-pulse`).

```bash
git clone https://github.com/nevermore23274/aethertune.git
cd aethertune
cargo build --release
./target/release/AetherTune
```

</details>

<details>
<summary><b>Windows</b></summary>

Download the latest `.zip` from the [Releases page](https://github.com/nevermore23274/AetherTune/releases). The zip includes `AetherTune.exe` and `mpv.exe` bundled together — no separate installation needed.

1. Extract the zip to a folder
2. Open **Windows Terminal** (recommended) and navigate to the folder
3. Run `AetherTune.exe`

> **Note:** For the best experience, use [Windows Terminal](https://aka.ms/terminal) rather than cmd.exe. The legacy console has limited support for keyboard input and ANSI rendering that TUI apps rely on.
>
> **Windows limitations:** Audio visualization uses a simulated mode (no real-time audio capture yet). Playback, station browsing, favorites, and all other features work normally.

</details>

## Usage

```bash
# Run normally (with CRT boot animation)
aethertune

# Skip the launch menu
aethertune --skip-menu

# Adjust boot animation speed (fast, normal, slow, off)
aethertune --boot-speed=fast
```

> On Windows, run `AetherTune.exe` from Windows Terminal. If installed from source on Linux, use `./target/release/AetherTune`.

## Keybindings

Below is a list of keyboard shortcuts. Press `?` in the app to see them as well (`Esc` closes the overlay).

| Key                    | Action                                       |
| ---------------------- | -------------------------------------------- |
| `↑` / `↓` or `j` / `k` | Navigate station list                        |
| `Enter`                | Play selected station                        |
| `s`                    | Stop playback                                |
| `+` / `-`              | Volume up / down                             |
| `/`                    | Search stations                              |
| `f`                    | Toggle favorite                              |
| `i`                    | Station details overlay                      |
| `n`                    | Load more stations                           |
| `Tab`                  | Cycle panel (Stations / Favorites / History) |
| `[` / `]`              | Cycle genre category                         |
| `Shift+Tab`            | Cycle genre category (backward)              |
| `?`                    | Help overlay                                 |
| `` ` ``                | Performance profiler                         |
| `<` / `>`              | Adjust tick rate (when profiler is open)     |
| `q`                    | Quit                                         |

## Settings

AetherTune has a settings screen accessible from the launch menu. Settings are persisted to `~/.aethertune/config.json`.

### Country Code

Set a two-letter ISO 3166-1 Alpha-2 country code (e.g. `US`, `DE`, `GB`, `JP`) to blend local stations into your results. When configured, roughly 30% of stations in each genre and search result will come from your country, interleaved naturally with global results sorted by popularity.

To configure: launch AetherTune → select **Settings** from the menu → type your two-letter country code → press **Enter** to save.

Leave the country code empty (backspace to clear) for pure global results — this is the default.

You can also edit the config file directly:

```json
{
  "tick_rate_ms": 30,
  "volume": 50,
  "country_code": "US"
}
```

## Architecture

```
src/
├── main.rs                   Entry point, event loop, frame timing
├── app.rs                    App state, business logic, perf stats
├── audio/
│   ├── player.rs             mpv playback, IPC, parec capture, stream info
│   ├── pipe.rs               FIFO creation, PCM reader thread, radix-2 FFT analysis
│   └── visualizer.rs         Bar animation (real + simulated modes)
├── storage/
│   ├── config.rs             User preferences (tick rate, volume, country code)
│   ├── favorites.rs          JSON persistence for favorites
│   └── history.rs            JSON persistence for play history
└── ui/
    ├── mod.rs                Layout orchestration
    ├── helpers.rs            Color palette, shared widgets
    ├── launcher.rs           CRT boot animation, start menu, settings screen
    ├── header.rs             Top bar (LIVE indicator, genre, hints)
    ├── station_list.rs       Left panel (stations/favorites/history)
    ├── now_playing.rs        Station info + session timer
    ├── song_log.rs           Rolling ICY metadata log
    ├── visualizer.rs         Spectrum bar rendering
    ├── stream_info.rs        Live stream health panel
    ├── media_browser.rs      Media source switcher (Radio/Subsonic stub)
    ├── overlays.rs           Help + station detail popups
    ├── shutdown.rs           CRT power-off animation on quit
    └── perf_overlay.rs       Built-in performance profiler
```

### Audio visualization pipeline

When `parec` is available, AetherTune captures audio through the PulseAudio/PipeWire monitor source:

1. **mpv** plays audio normally through the default audio output
2. **parec** captures the monitor source and writes raw s16le stereo 48kHz PCM to a named FIFO
3. A background thread reads the FIFO with minimal buffering (one 4KB chunk ≈ 21ms of audio) and runs an **in-place radix-2 Cooley-Tukey FFT** with Hann windowing — producing 512 frequency bins that are grouped into 16 logarithmically-spaced bands (50Hz–10kHz). The FFT, window coefficients, and band edges are all pre-allocated at thread startup for zero per-frame heap allocation.
4. Band energies and RMS are pushed to a shared `Arc<Mutex<AudioAnalysis>>`
5. The visualizer applies CAVA-inspired post-processing: gravity fall-off (accelerating drop), integral smoothing (weighted running average), and automatic sensitivity adjustment

Process isolation is handled carefully: `parec` runs in its own process group via `setsid()`, and cleanup uses `kill(-pgid, SIGTERM)` to ensure no orphaned processes.

### Data persistence

Favorites, history, and user preferences (tick rate, volume, country code) are stored as JSON in `~/.aethertune/`. The serializer/parser is hand-rolled (no serde dependency) to keep the dependency tree minimal. Settings like tick rate are saved automatically when adjusted and restored on next launch. The country code is configured via the Settings screen in the launch menu.

## License

MIT