// ── Media Browser Panel ───────────────────────────────────────────────
//
// Bottom-right panel: shows which source the left panel is browsing
// (Radio / Subsonic, toggled with the toggle_source key) and, for
// Subsonic, the play queue — the current track and what's up next.

use crate::core::app::App;
use crate::core::types::MediaSource;
use crate::storage::config::keycode_to_string;
use super::helpers::truncate_str;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
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
        .style(Style::default().bg(app.theme.bg_panel));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    let switch_key = keycode_to_string(app.keybindings.toggle_source.primary);
    let mut lines = vec![source_tabs(app, &switch_key), Line::from("")];
    let width = inner.width as usize;

    match app.source {
        MediaSource::Radio => {
            lines.push(dim_line(&format!("  {} → browse your Subsonic library", switch_key)));
            lines.push(dim_line("  Use / to search radio stations"));
        }
        MediaSource::Subsonic => {
            let room = (inner.height as usize).saturating_sub(lines.len());
            lines.extend(queue_lines(app, room, width));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn source_tabs(app: &App, switch_key: &str) -> Line<'static> {
    let tab = |label: &str, active: bool| {
        if active {
            Span::styled(
                format!(" ● {} ", label),
                Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" ○ {} ", label), Style::default().fg(Color::Rgb(70, 70, 100)))
        }
    };
    Line::from(vec![
        Span::raw(" "),
        tab("Radio", app.source == MediaSource::Radio),
        Span::styled("│", Style::default().fg(Color::Rgb(60, 60, 100))),
        tab("Subsonic", app.source == MediaSource::Subsonic),
        Span::styled(format!("   ({} to switch)", switch_key), Style::default().fg(Color::Rgb(60, 60, 90))),
    ])
}

/// Current track and as many upcoming ones as fit in `room` lines.
fn queue_lines(app: &App, room: usize, width: usize) -> Vec<Line<'static>> {
    let Some(queue) = app.subsonic.active_queue(&app.player) else {
        return vec![
            dim_line("  Queue empty"),
            dim_line("  Pick a song and press Enter to play from it"),
        ];
    };
    let Some(current) = queue.current_index(&app.player) else {
        return vec![dim_line("  Starting…")];
    };

    let songs = queue.songs();
    let budget = width.saturating_sub(6);
    let mut lines = Vec::new();
    for (i, song) in songs.iter().enumerate().skip(current).take(room) {
        let label = match &song.artist {
            Some(artist) => format!("{} — {}", song.title, artist),
            None => song.title.clone(),
        };
        let is_current = i == current;
        lines.push(Line::from(vec![
            Span::styled(
                if is_current { "  ▶ " } else { "    " },
                Style::default().fg(app.theme.positive),
            ),
            Span::styled(
                truncate_str(&label, budget),
                if is_current {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.text_muted)
                },
            ),
        ]));
    }
    let remaining = songs.len().saturating_sub(current + lines.len());
    if remaining > 0 && lines.len() == room && room > 1 {
        lines.pop();
        lines.push(dim_line(&format!("    … {} more", remaining + 1)));
    }
    lines
}

fn dim_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), Style::default().fg(Color::Rgb(70, 70, 100))))
}
