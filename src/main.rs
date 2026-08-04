pub mod audio;
pub mod core;
pub mod input;
pub mod storage;
pub mod ui;

use crate::core::perf::FrameTiming;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Resolve the storage directory before anything else touches it — the
    // boot menu's Settings screen calls Config::load() before App::new runs.
    storage::paths::resolve_and_set(&args);

    let skip_menu = args.iter().any(|a| a == "--skip-menu" || a == "-s");

    // Parse boot speed: --boot-speed=fast|normal|slow|off (default: normal)
    let boot_speed = args
        .iter()
        .find(|a| a.starts_with("--boot-speed"))
        .and_then(|a| a.strip_prefix("--boot-speed="))
        .unwrap_or("normal");

    let speed = match boot_speed {
        "fast" => ui::launcher::BootSpeed::Fast,
        "slow" => ui::launcher::BootSpeed::Slow,
        "off" => ui::launcher::BootSpeed::Off,
        _ => ui::launcher::BootSpeed::Normal,
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    // Show launch menu unless --skip-menu was passed
    if !skip_menu {
        match ui::launcher::show(&mut terminal, speed) {
            Ok(true) => {} // User chose "Start Radio"
            Ok(false) => {
                // User chose "Quit"
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;
                return Ok(());
            }
            Err(e) => {
                disable_raw_mode()?;
                execute!(
                    terminal.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                terminal.show_cursor()?;
                return Err(e.into());
            }
        }
    }

    // Construct the app immediately with an empty station list.
    // The initial fetch runs in the background — stations appear once it completes.
    let mut app = core::app::App::new(Vec::new());
    // If visualizer is disabled, start in low-power mode
    if !app.visualizer_enabled {
        app.tick_rate_ms = 200;
    }
    app.start_initial_fetch();

    let mut last_tick = Instant::now();

    loop {
        let frame_start = Instant::now();

        // ── Draw ──────────────────────────────────────────────────
        let draw_start = Instant::now();
        terminal.draw(|f| ui::draw(f, &app))?;
        let draw_us = draw_start.elapsed().as_micros() as u64;

        // ── Event handling ────────────────────────────────────────
        let tick_rate = Duration::from_millis(app.tick_rate_ms);
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Measure the idle wait separately from event handling work
        let wait_start = Instant::now();
        let has_event = crossterm::event::poll(timeout)?;
        let event_wait_us = wait_start.elapsed().as_micros() as u64;

        let handle_start = Instant::now();
        if has_event {
            if let Event::Key(key) = event::read()? {
                // On Windows, crossterm sends both Press and Release events.
                // Only act on Press to avoid double-firing every keystroke.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let should_quit = input::handler::handle_key(&mut app, key.code, key.modifiers);
                if should_quit {
                    break;
                }
            }
        }
        let event_handle_us = handle_start.elapsed().as_micros() as u64;

        // ── Tick: poll mpv IPC and update visualizer ──────────────
        let mut poll_us = 0u64;
        let mut vis_us = 0u64;
        let mut had_tick = false;

        if last_tick.elapsed() >= tick_rate {
            had_tick = true;
            let poll_start = Instant::now();
            app.player.poll();
            app.check_song_change();

            // Check if a background station fetch has completed
            app.poll_fetch();

            // Update FFT rate measurement for profiler
            app.update_fft_rate();

            poll_us = poll_start.elapsed().as_micros() as u64;

            let vis_start = Instant::now();
            if app.visualizer_enabled {
                if app.player.has_real_audio() {
                    let used_real = app.visualizer.tick_real(&app.analysis, app.volume);
                    if !used_real {
                        app.visualizer.tick_simulated(app.player.is_playing(), app.player.audio_level, app.volume);
                    }
                } else {
                    let level = app.player.audio_level;
                    app.visualizer.tick_simulated(app.player.is_playing(), level, app.volume);
                }
            }
            vis_us = vis_start.elapsed().as_micros() as u64;

            last_tick = Instant::now();
        }

        // ── Record frame timing ───────────────────────────────────
        let total_us = frame_start.elapsed().as_micros() as u64;
        let tick_budget_us = app.tick_rate_ms * 1000;
        app.perf.record(FrameTiming {
            draw_us,
            event_wait_us,
            event_handle_us,
            poll_us,
            vis_us,
            total_us,
            had_tick,
        }, tick_budget_us);
    }

    // Stop playback before the shutdown animation
    app.stop();

    // CRT power-off animation
    ui::shutdown::play(&mut terminal)?;

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}