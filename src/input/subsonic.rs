use crossterm::event::KeyCode;

use crate::core::app::App;
use crate::core::types::InputMode;

/// Keys for the Subsonic browser (left panel), checked before the global
/// keys whenever Subsonic is the active source. Returns true if the key was
/// handled here. Radio-only keys are swallowed so they can't start station
/// fetches from the Subsonic view.
pub fn handle_browser_key(app: &mut App, code: KeyCode) -> bool {
    let kb = &app.keybindings;

    if kb.navigate_down.matches(code) {
        app.subsonic.select_next();
    } else if kb.navigate_up.matches(code) {
        app.subsonic.select_previous();
    } else if kb.play.matches(code) {
        app.subsonic_activate();
    } else if kb.back.matches(code) {
        app.subsonic.back();
    } else if kb.cycle_panel.matches(code) {
        app.subsonic.cycle_tab();
    } else if kb.search.matches(code) {
        app.search_query.clear();
        app.input_mode = InputMode::Editing;
    } else if kb.load_more.matches(code) {
        app.subsonic.load_more();
    } else if kb.genre_next.matches(code)
        || kb.genre_prev.matches(code)
        || kb.genre_picker.matches(code)
        || kb.toggle_favorite.matches(code)
        || kb.station_detail.matches(code)
        || code == KeyCode::BackTab
    {
        // Radio-only actions: no-op here
    } else {
        return false;
    }
    true
}

/// Music playback keys. These work from either source's view, so music
/// can be controlled while browsing radio. Returns true if handled.
pub fn handle_playback_key(app: &mut App, code: KeyCode) -> bool {
    let kb = &app.keybindings;

    if kb.pause.matches(code) {
        app.subsonic.toggle_pause(&mut app.player);
    } else if kb.next_track.matches(code) {
        app.subsonic.next_track(&mut app.player);
    } else if kb.prev_track.matches(code) {
        app.subsonic.previous_track(&mut app.player);
    } else if kb.seek_forward.matches(code) {
        app.subsonic.seek(&mut app.player, crate::subsonic::session::SEEK_STEP_SECS);
    } else if kb.seek_back.matches(code) {
        app.subsonic.seek(&mut app.player, -crate::subsonic::session::SEEK_STEP_SECS);
    } else {
        return false;
    }
    true
}
