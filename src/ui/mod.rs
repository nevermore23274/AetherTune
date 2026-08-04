pub mod header;
pub mod helpers;
pub mod genre_picker;
pub mod launcher;
pub mod media_browser;
pub mod now_playing;
pub mod overlays;
pub mod perf_overlay;
pub mod settings;
pub mod shutdown;
pub mod song_log;
pub mod station_list;
pub mod stream_info;
pub mod themes;
pub mod theme_picker;
pub mod visualizer;

use crate::core::app::App;
use crate::core::types::Overlay;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Block,
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.size();

    // Fill background with theme color
    f.render_widget(
        Block::default().style(Style::default().bg(app.theme.bg_dark)),
        size,
    );

    // ── Main Layout: header / body ────────────────────────────────
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header bar
            Constraint::Min(10),   // Body
        ])
        .split(size);

    header::draw(f, app, main_chunks[0]);
    draw_body(f, app, main_chunks[1]);

    // ── Overlays ───────────────────────────────────────────────────
    match &app.overlay {
        Overlay::Help => overlays::draw_help(f, app, size),
        Overlay::StationDetail => overlays::draw_detail(f, app, size),
        Overlay::Settings => settings::draw(f, app, size),
        Overlay::GenrePicker => genre_picker::draw(f, app, size),
        Overlay::ThemePicker => theme_picker::draw(f, app, size),
        Overlay::None => {}
    }

    // Perf overlay renders on top of everything (independent of Overlay enum)
    if app.show_perf {
        perf_overlay::draw(f, app, size);
    }
}

fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    station_list::draw(f, app, body_chunks[0]);

    // Right side: now playing / (song log + vis column) / media browser
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),  // Now playing info + clock
            Constraint::Min(6),     // Song log + (visualizer / stream info)
            Constraint::Length(8),  // Media browser stub
        ])
        .split(body_chunks[1]);

    now_playing::draw(f, app, right_chunks[0]);

    // Middle row: song log on left, visualizer + stream info stacked on right
    if app.visualizer_enabled {
        let middle_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Song log
                Constraint::Percentage(40), // Visualizer + stream info column
            ])
            .split(right_chunks[1]);

        song_log::draw(f, app, middle_chunks[0]);

        // Right column: visualizer on top, stream info below
        let vis_column = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50), // Visualizer
                Constraint::Percentage(50), // Stream info
            ])
            .split(middle_chunks[1]);

        visualizer::draw(f, app, vis_column[0]);
        stream_info::draw(f, app, vis_column[1]);
    } else {
        // Visualizer disabled — song log gets full width, stream info below
        let middle_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70), // Song log (wider)
                Constraint::Percentage(30), // Stream info only
            ])
            .split(right_chunks[1]);

        song_log::draw(f, app, middle_chunks[0]);
        stream_info::draw(f, app, middle_chunks[1]);
    }

    // Bottom: media browser
    media_browser::draw(f, app, right_chunks[2]);
}