use crossterm::event::{KeyCode, KeyModifiers};

use crate::core::app::App;
use crate::core::types::{InputMode, Overlay};
use crate::storage::config::KeyBindings;

/// Handle a keypress in normal mode. Returns true if the app should quit.
pub fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    match app.input_mode {
        InputMode::Normal => handle_normal(app, code, modifiers),
        InputMode::Editing => {
            handle_editing(app, code);
            false
        }
    }
}

/// Handle a keypress in editing (search) mode.
fn handle_editing(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.perform_search();
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
    }
}

/// Handle a keypress in normal mode. Returns true if the app should quit.
fn handle_normal(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    // ── Theme picker overlay ──
    if app.overlay == Overlay::ThemePicker {
        handle_theme_picker(app, code);
        return false;
    }

    // ── Genre picker overlay ──
    if app.overlay == Overlay::GenrePicker {
        handle_genre_picker(app, code);
        return false;
    }

    // ── Settings overlay has its own input handling ──
    if app.overlay == Overlay::Settings {
        handle_settings(app, code);
        return false;
    }

    // Handle other overlays (help, detail)
    if app.overlay != Overlay::None {
        match code {
            KeyCode::Esc => {
                app.overlay = Overlay::None;
            }
            _ if app.keybindings.help.matches(code) => {
                app.overlay = Overlay::None;
            }
            _ if app.keybindings.station_detail.matches(code) => {
                app.overlay = Overlay::None;
            }
            _ => {}
        }
        return false;
    }

    // ── Normal mode: use configured keybindings ──
    let kc = code;

    if app.keybindings.quit.matches(kc) {
        return true;
    } else if app.keybindings.help.matches(kc) {
        app.overlay = Overlay::Help;
    } else if app.keybindings.station_detail.matches(kc) {
        app.overlay = Overlay::StationDetail;
    } else if app.keybindings.settings.matches(kc) {
        app.overlay = Overlay::Settings;
        app.settings_awaiting_key = None;
    } else if app.keybindings.genre_picker.matches(kc) {
        app.genre_selected = app.category_index;
        app.overlay = Overlay::GenrePicker;
    } else if app.keybindings.theme_picker.matches(kc) {
        // Open theme picker overlay
        let themes = crate::ui::themes::Theme::all();
        app.theme_selected = themes.iter()
            .position(|t| t.name == app.theme.name)
            .unwrap_or(0);
        app.overlay = Overlay::ThemePicker;
    } else if app.keybindings.visualizer_toggle.matches(kc) {
        app.visualizer_enabled = !app.visualizer_enabled;
        app.player.visualizer_enabled = app.visualizer_enabled;
        if !app.visualizer_enabled {
            // Stop audio capture and drop to low-power tick rate
            app.player.stop_capture_if_running();
            app.tick_rate_ms = 200; // 5 FPS — plenty for static UI
        } else {
            // Restore user's configured tick rate from config
            let config = crate::storage::config::Config::load();
            app.tick_rate_ms = config.tick_rate_ms;
            if app.player.is_playing() {
                app.player.restart_capture();
            }
        }
        app.save_config();
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
        if modifiers.contains(KeyModifiers::SHIFT) {
            app.switch_category();
        } else {
            app.cycle_panel();
        }
    } else if kc == KeyCode::BackTab {
        app.switch_category_back();
    } else if app.keybindings.genre_prev.matches(kc) {
        app.switch_category_back();
    } else if app.keybindings.genre_next.matches(kc) {
        app.switch_category();
    } else if app.keybindings.load_more.matches(kc) {
        app.load_more();
    } else if app.keybindings.perf_toggle.matches(kc) {
        app.show_perf = !app.show_perf;
    } else if app.show_perf && app.keybindings.perf_tick_slower.matches(kc) {
        app.tick_rate_ms = (app.tick_rate_ms + 10).min(200);
        app.save_config();
    } else if app.show_perf && app.keybindings.perf_tick_faster.matches(kc) {
        app.tick_rate_ms = app.tick_rate_ms.saturating_sub(10).max(10);
        app.save_config();

    // Smoothing adjustment (only when profiler is open)
    } else if app.show_perf && kc == KeyCode::Char('{') {
        // Decrease smoothing (more responsive)
        let nr = app.visualizer.noise_reduction;
        app.visualizer.noise_reduction = ((nr - 0.05) * 100.0).round() / 100.0;
        if app.visualizer.noise_reduction < 0.05 {
            app.visualizer.noise_reduction = 0.05;
        }
    } else if app.show_perf && kc == KeyCode::Char('}') {
        // Increase smoothing (smoother)
        let nr = app.visualizer.noise_reduction;
        app.visualizer.noise_reduction = ((nr + 0.05) * 100.0).round() / 100.0;
        if app.visualizer.noise_reduction > 0.95 {
            app.visualizer.noise_reduction = 0.95;
        }
    }

    false
}

fn handle_theme_picker(app: &mut App, code: KeyCode) {
    let themes = crate::ui::themes::Theme::all();
    let total = themes.len();
    match code {
        KeyCode::Esc => {
            app.overlay = Overlay::None;
        }
        _ if app.keybindings.theme_picker.matches(code) => {
            app.overlay = Overlay::None;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.theme_selected > 0 {
                app.theme_selected -= 1;
                // Live preview
                app.theme = themes.into_iter().nth(app.theme_selected).unwrap();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.theme_selected < total - 1 {
                app.theme_selected += 1;
                // Live preview
                app.theme = themes.into_iter().nth(app.theme_selected).unwrap();
            }
        }
        KeyCode::Enter => {
            app.theme = themes.into_iter().nth(app.theme_selected).unwrap();
            app.save_config();
            app.overlay = Overlay::None;
        }
        _ => {}
    }
}

fn handle_genre_picker(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.overlay = Overlay::None;
        }
        _ if app.keybindings.genre_picker.matches(code) => {
            app.overlay = Overlay::None;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.genre_selected > 0 {
                app.genre_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.genre_selected < app.categories.len() - 1 {
                app.genre_selected += 1;
            }
        }
        KeyCode::Enter => {
            app.select_genre(app.genre_selected);
            app.overlay = Overlay::None;
        }
        _ => {}
    }
}

fn handle_settings(app: &mut App, code: KeyCode) {
    // If awaiting a key for rebinding
    if let Some((action_idx, is_alt)) = app.settings_awaiting_key {
        match code {
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
        return;
    }

    // Normal settings navigation
    match code {
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