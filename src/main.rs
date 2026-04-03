pub mod app;
pub mod audio;
pub mod storage;
pub mod ui;

use app::{FrameTiming, InputMode, Overlay};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
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

    // Fetch initial stations — request a larger pool sorted by popularity,
    // hide broken streams, and let the API return the best results first.
    let client = radiobrowser::RadioBrowserAPI::new().await?;
    let mut stations_data = client
        .get_stations()
        .tag("lo-fi")
        .order(radiobrowser::StationOrder::Votes)
        .reverse(true)
        .hidebroken(true)
        .limit("250")
        .send()
        .await?;

    // Filter out spam stations (>50K votes are likely botted)
    stations_data.retain(|s| s.votes < 50_000);

    let mut app = app::App::new(stations_data);

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
                match app.input_mode {
                    InputMode::Normal => {
                        // Handle overlays first
                        if app.overlay != Overlay::None {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('i') => {
                                    app.overlay = Overlay::None;
                                }
                                _ => {}
                            }
                            continue;
                        }

                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('?') => {
                                app.overlay = Overlay::Help;
                            }
                            KeyCode::Char('i') => {
                                app.overlay = Overlay::StationDetail;
                            }
                            KeyCode::Char('/') => {
                                app.search_query.clear();
                                app.input_mode = InputMode::Editing;
                            }
                            KeyCode::Char('s') => app.stop(),
                            KeyCode::Char('f') => app.toggle_favorite(),
                            KeyCode::Char('+') | KeyCode::Char('=') => app.set_volume(5),
                            KeyCode::Char('-') => app.set_volume(-5),
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Enter => app.play(),
                            KeyCode::Tab => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.switch_category().await?;
                                } else {
                                    app.cycle_panel();
                                }
                            }
                            KeyCode::BackTab => {
                                app.switch_category_back().await?;
                            }
                            KeyCode::Char('[') => {
                                app.switch_category_back().await?;
                            }
                            KeyCode::Char(']') => {
                                app.switch_category().await?;
                            }
                            KeyCode::Char('n') => {
                                app.load_more().await?;
                            }
                            // Perf overlay toggle
                            KeyCode::Char('`') => {
                                app.show_perf = !app.show_perf;
                            }
                            // Tick rate adjustment (only when perf overlay is shown)
                            KeyCode::Char('<') | KeyCode::Char(',') if app.show_perf => {
                                app.tick_rate_ms = (app.tick_rate_ms + 10).min(200);
                                app.save_config();
                            }
                            KeyCode::Char('>') | KeyCode::Char('.') if app.show_perf => {
                                app.tick_rate_ms = app.tick_rate_ms.saturating_sub(10).max(10);
                                app.save_config();
                            }
                            _ => {}
                        }
                    }
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                            app.perform_search().await?;
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        _ => {}
                    },
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
            poll_us = poll_start.elapsed().as_micros() as u64;

            let vis_start = Instant::now();
            if app.player.has_real_audio() {
                let used_real = app.visualizer.tick_real(&app.analysis, app.volume);
                if !used_real {
                    app.visualizer.tick_simulated(app.player.is_playing(), app.player.audio_level, app.volume);
                }
            } else {
                let level = app.player.audio_level;
                app.visualizer.tick_simulated(app.player.is_playing(), level, app.volume);
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