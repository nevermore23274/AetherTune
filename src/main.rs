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
use storage::config::KeyBindings;
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

    // Load config to check for country code
    let init_config = storage::config::Config::load();
    let country_code = init_config.country_code.clone();

    // Fetch initial stations — global pool sorted by popularity
    let client = radiobrowser::RadioBrowserAPI::new().await?;
    let mut global = client
        .get_stations()
        .tag("lo-fi")
        .order(radiobrowser::StationOrder::Votes)
        .reverse(true)
        .hidebroken(true)
        .limit("175")
        .send()
        .await?;
    global.retain(|s| s.votes < 50_000);

    // Blend in local stations if country code is configured
    let stations_data = if !country_code.is_empty() {
        let client2 = radiobrowser::RadioBrowserAPI::new().await?;
        let mut local = client2
            .get_stations()
            .tag("lo-fi")
            .countrycode(&country_code)
            .order(radiobrowser::StationOrder::Votes)
            .reverse(true)
            .hidebroken(true)
            .limit("75")
            .send()
            .await?;
        local.retain(|s| s.votes < 50_000);
        app::App::interleave_static(global, local)
    } else {
        global
    };

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
                        // ── Settings overlay has its own input handling ──
                        if app.overlay == Overlay::Settings {
                            // If awaiting a key for rebinding
                            if let Some((action_idx, is_alt)) = app.settings_awaiting_key {
                                match key.code {
                                    KeyCode::Esc => {
                                        // Cancel the rebind
                                        app.settings_awaiting_key = None;
                                    }
                                    new_key => {
                                        if let Some(json_key) = app.keybindings.key_at_index(action_idx) {
                                            let json_key = json_key.to_string();
                                            if is_alt {
                                                // Set alt, keep primary
                                                let actions = app.keybindings.all_actions();
                                                let primary = actions[action_idx].2.primary;
                                                app.keybindings.set_binding(&json_key, primary, Some(new_key));
                                            } else {
                                                // Set primary, keep alt
                                                let actions = app.keybindings.all_actions();
                                                let alt = actions[action_idx].2.alt;
                                                app.keybindings.set_binding(&json_key, new_key, alt);
                                            }
                                            app.save_config();
                                        }
                                        app.settings_awaiting_key = None;
                                    }
                                }
                            } else {
                                // Normal settings navigation
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('S') => {
                                        app.overlay = Overlay::None;
                                    }
                                    KeyCode::Up | KeyCode::Char('k') => {
                                        if app.settings_selected > 0 {
                                            app.settings_selected -= 1;
                                        }
                                    }
                                    KeyCode::Down | KeyCode::Char('j') => {
                                        let count = app.keybindings.all_actions().len();
                                        if app.settings_selected < count - 1 {
                                            app.settings_selected += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        // Start rebinding primary key
                                        app.settings_awaiting_key = Some((app.settings_selected, false));
                                    }
                                    KeyCode::Char('a') => {
                                        // Start rebinding alt key
                                        app.settings_awaiting_key = Some((app.settings_selected, true));
                                    }
                                    KeyCode::Char('d') => {
                                        // Clear the alt binding
                                        if let Some(json_key) = app.keybindings.key_at_index(app.settings_selected) {
                                            let json_key = json_key.to_string();
                                            let actions = app.keybindings.all_actions();
                                            let primary = actions[app.settings_selected].2.primary;
                                            app.keybindings.set_binding(&json_key, primary, None);
                                            app.save_config();
                                        }
                                    }
                                    KeyCode::Char('r') => {
                                        // Reset this action to default
                                        let defaults = KeyBindings::default();
                                        let default_actions = defaults.all_actions();
                                        if let Some((json_key, _, def_binding)) = default_actions.get(app.settings_selected) {
                                            let json_key = json_key.to_string();
                                            app.keybindings.set_binding(&json_key, def_binding.primary, def_binding.alt);
                                            app.save_config();
                                        }
                                    }
                                    KeyCode::Char('R') => {
                                        // Reset ALL to defaults
                                        app.keybindings = KeyBindings::default();
                                        app.save_config();
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }

                        // Handle other overlays (help, detail)
                        if app.overlay != Overlay::None {
                            match key.code {
                                KeyCode::Esc => {
                                    app.overlay = Overlay::None;
                                }
                                _ if app.keybindings.help.matches(key.code) => {
                                    app.overlay = Overlay::None;
                                }
                                _ if app.keybindings.station_detail.matches(key.code) => {
                                    app.overlay = Overlay::None;
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // ── Normal mode: use configured keybindings ──
                        let kc = key.code;

                        if app.keybindings.quit.matches(kc) {
                            break;
                        } else if app.keybindings.help.matches(kc) {
                            app.overlay = Overlay::Help;
                        } else if app.keybindings.station_detail.matches(kc) {
                            app.overlay = Overlay::StationDetail;
                        } else if app.keybindings.settings.matches(kc) {
                            app.overlay = Overlay::Settings;
                            app.settings_awaiting_key = None;
                        } else if app.keybindings.search.matches(kc) {
                            app.search_query.clear();
                            app.input_mode = InputMode::Editing;
                        } else if app.keybindings.stop.matches(kc) {
                            app.stop();
                        } else if app.keybindings.toggle_favorite.matches(kc) {
                            app.toggle_favorite();
                        } else if app.keybindings.volume_up.matches(kc) {
                            app.set_volume(5);
                        } else if app.keybindings.volume_down.matches(kc) {
                            app.set_volume(-5);
                        } else if app.keybindings.navigate_down.matches(kc) {
                            app.next();
                        } else if app.keybindings.navigate_up.matches(kc) {
                            app.previous();
                        } else if app.keybindings.play.matches(kc) {
                            app.play();
                        } else if app.keybindings.cycle_panel.matches(kc) {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.switch_category().await?;
                            } else {
                                app.cycle_panel();
                            }
                        } else if kc == KeyCode::BackTab {
                            app.switch_category_back().await?;
                        } else if app.keybindings.genre_prev.matches(kc) {
                            app.switch_category_back().await?;
                        } else if app.keybindings.genre_next.matches(kc) {
                            app.switch_category().await?;
                        } else if app.keybindings.load_more.matches(kc) {
                            app.load_more().await?;
                        } else if app.keybindings.perf_toggle.matches(kc) {
                            app.show_perf = !app.show_perf;
                        } else if app.show_perf && app.keybindings.perf_tick_slower.matches(kc) {
                            app.tick_rate_ms = (app.tick_rate_ms + 10).min(200);
                            app.save_config();
                        } else if app.show_perf && app.keybindings.perf_tick_faster.matches(kc) {
                            app.tick_rate_ms = app.tick_rate_ms.saturating_sub(10).max(10);
                            app.save_config();
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