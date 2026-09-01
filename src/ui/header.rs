use crate::core::app::App;
use crate::core::types::{ActivePanel, InputMode};

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
            Span::styled(" 🔍 > ", Style::default().fg(app.theme.text_warn)),
            Span::styled(
                format!("{}_", app.search_query),
                Style::default().fg(app.theme.text_warn),
            ),
        ]);

        let block = header_block(&app.theme);
        let paragraph = Paragraph::new(search_line)
            .block(block)
            .style(Style::default().bg(app.theme.bg_panel));
        f.render_widget(paragraph, area);
    } else {
        // Normal mode: LIVE indicator + genre + hints
        let playing_indicator = if app.player.is_playing() {
            Span::styled(
                " ▶ LIVE ",
                Style::default()
                    .fg(Color::Black)
                    .bg(app.theme.positive)
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

        // Genre only means something on the Stations panel — Favorites/History
        // aren't filtered by it, so showing it there just implies a filter
        // that isn't actually being applied.
        let panel_label = if app.active_panel == ActivePanel::Stations {
            format!("  Genre: {}", cat)
        } else {
            format!("  Panel: {}", app.active_panel.as_str())
        };

        let line = Line::from(vec![
            Span::styled(" ", Style::default()),
            playing_indicator,
            Span::styled(
                panel_label,
                Style::default().fg(app.theme.accent),
            ),
            Span::styled(
                app.header_hint.as_str(),
                Style::default().fg(Color::Rgb(80, 80, 110)),
            ),
            Span::styled(
                app.header_vis_status.as_str(),
                Style::default().fg(app.theme.text_warn),
            ),
        ]);

        let block = header_block(&app.theme);
        let paragraph = Paragraph::new(line)
            .block(block)
            .style(Style::default().bg(app.theme.bg_panel));
        f.render_widget(paragraph, area);
    }
}

fn header_block(theme: &crate::ui::themes::Theme) -> Block<'static> {
    Block::default()
        .title(Line::from(vec![
            Span::styled(" 🎵 ", Style::default().fg(theme.secondary)),
            Span::styled(
                "AetherTune",
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", Style::default()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .style(Style::default().bg(theme.bg_panel))
}