use crate::app::{App, InputMode};
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    if app.input_mode == InputMode::Editing {
        // Search mode: show search input
        let search_line = Line::from(vec![
            Span::styled(" 🔍 > ", Style::default().fg(YELLOW)),
            Span::styled(
                format!("{}_", app.search_query),
                Style::default().fg(YELLOW),
            ),
        ]);

        let block = header_block();
        let paragraph = Paragraph::new(search_line)
            .block(block)
            .style(Style::default().bg(PANEL_BG));
        f.render_widget(paragraph, area);
    } else {
        // Normal mode: LIVE indicator + genre + hints
        let playing_indicator = if app.player.is_playing() {
            Span::styled(
                " ▶ LIVE ",
                Style::default()
                    .fg(Color::Black)
                    .bg(NEON_GREEN)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                " ■ IDLE ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(80, 80, 100)),
            )
        };

        let cat = app.categories[app.category_index];

        let line = Line::from(vec![
            Span::styled(" ", Style::default()),
            playing_indicator,
            Span::styled(
                format!("  Genre: {}", cat),
                Style::default().fg(CYAN),
            ),
            Span::styled(
                "  │  / search  │  ? help",
                Style::default().fg(Color::Rgb(80, 80, 110)),
            ),
        ]);

        let block = header_block();
        let paragraph = Paragraph::new(line)
            .block(block)
            .style(Style::default().bg(PANEL_BG));
        f.render_widget(paragraph, area);
    }
}

fn header_block() -> Block<'static> {
    Block::default()
        .title(Line::from(vec![
            Span::styled(" 🎵 ", Style::default().fg(MAGENTA)),
            Span::styled(
                "AetherTune",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .style(Style::default().bg(PANEL_BG))
}