// ── Media Browser Panel ───────────────────────────────────────────────
//
// CURRENT STATE: Stub UI only. Shows Radio/Subsonic tabs but no interaction.
//
// FUTURE PLANS — Subsonic Integration:
//
//   This panel will become a unified media browser that lets the user
//   search and play from both internet radio (radiobrowser API, already
//   working) and a personal Subsonic music server.
//
//   Backend: Targets Subsonic (original) servers. We'll need a new
//   `subsonic` module under `src/audio/` or a dedicated `src/subsonic/` 
//   that implements:
//     - Authentication (user/pass/token via Subsonic REST API)
//     - Library browsing (artists, albums, playlists, random)
//     - Search (search3 endpoint)
//     - Stream URL generation (stream.view endpoint → feed to mpv)
//     - Config persistence (~/.aethertune/subsonic.json: server URL, creds)
//
//   UI considerations:
//     - Tab switching: Need a new keybinding to toggle the active source
//       in this panel (e.g. `S` for Subsonic, or left/right arrows when
//       this panel is focused). Currently there's no way to focus this
//       panel — Tab cycles Stations/Favorites/History in the left panel.
//       Options: (a) add MediaBrowser as a new ActivePanel variant so Tab
//       includes it, (b) dedicate a key like `m` to toggle the media
//       browser source, (c) make the source tabs focusable with their own
//       key when the panel is selected.
//     - When Subsonic tab is active: show search bar + results list
//       (albums, tracks). Selecting a track queues or plays it via mpv
//       using the Subsonic stream URL. The Now Playing / Song Log /
//       Stream Info panels should work unchanged since mpv handles both.
//     - When Radio tab is active: could mirror the station search that
//       currently lives in the header bar, or just show "use / to search".
//     - The media browser might need to grow taller once it has real
//       content — currently 8 rows, may need 12+ for search results.
//
//   Integration with existing player:
//     - mpv already handles HTTP streams, so Subsonic stream URLs should
//       just work via player.play_url(). The visualizer, stream info, and
//       song log will all work automatically.
//     - NowPlaying struct may need a `source: MediaSource` enum field
//       (Radio / Subsonic) so the UI can show appropriate metadata.
//     - Song log filtering (is_stream_noise) may need tweaks for Subsonic
//       stream URL patterns.
//
// ──────────────────────────────────────────────────────────────────────

use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, _app: &App, area: Rect) {
    // Source tabs
    let radio_tab = Span::styled(
        " ● Radio ",
        Style::default()
            .fg(NEON_GREEN)
            .add_modifier(Modifier::BOLD),
    );
    let subsonic_tab = Span::styled(
        " ○ Subsonic ",
        Style::default().fg(Color::Rgb(60, 60, 90)),
    );

    let title_line = Line::from(vec![
        Span::styled(" ", Style::default()),
        radio_tab,
        Span::styled("│", Style::default().fg(Color::Rgb(60, 60, 100))),
        subsonic_tab,
        Span::styled(" ", Style::default()),
    ]);

    let block = Block::default()
        .title(Span::styled(
            " Media Browser ",
            Style::default()
                .fg(Color::Rgb(100, 180, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    let lines = vec![
        title_line,
        Line::from(""),
        Line::from(Span::styled(
            "  Subsonic integration coming soon…",
            Style::default().fg(Color::Rgb(60, 60, 80)),
        )),
        Line::from(Span::styled(
            "  Use / to search radio stations",
            Style::default().fg(Color::Rgb(50, 50, 70)),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}