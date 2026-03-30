use crate::app::{ActivePanel, App};
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
    layout::Rect,
};

pub fn draw_help(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 70, area);
    f.render_widget(Clear, popup);

    let help_text = vec![
        Line::from(Span::styled(
            "⌨  Keybindings",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("↑/↓ or j/k", "Navigate station list"),
        help_line("Enter", "Play selected station"),
        help_line("s", "Stop playback"),
        help_line("+ / -", "Volume up / down"),
        help_line("/", "Search stations"),
        help_line("f", "Toggle favorite"),
        help_line("i", "Station details"),
        help_line("n", "Load more stations"),
        help_line("Tab", "Cycle panel (Stations/Favorites/History)"),
        help_line("[ / ]", "Cycle genre category"),
        help_line("Shift+Tab", "Cycle genre category (backward)"),
        help_line("?", "Toggle this help"),
        help_line("`", "Performance profiler"),
        help_line("< / >", "Adjust tick rate (profiler open)"),
        help_line("q", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? or Esc to close",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " Help ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(YELLOW))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup);
}

pub fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(65, 55, area);
    f.render_widget(Clear, popup);

    let station = match app.active_panel {
        ActivePanel::Stations => app.stations.get(app.selected_index),
        _ => None,
    };

    let lines = if let Some(s) = station {
        let fav = if app.is_favorite(&s.url) { "★ Yes" } else { "No" };
        vec![
            Line::from(Span::styled(
                s.name.clone(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            info_line("Genre", &s.tags, MAGENTA),
            info_line("Country", &s.country, CYAN),
            info_line("Bitrate", &format!("{} kbps", s.bitrate), YELLOW),
            info_line("Codec", &s.codec, ORANGE),
            info_line("Votes", &s.votes.to_string(), NEON_GREEN),
            info_line("Favorite", fav, YELLOW),
            Line::from(""),
            info_line("URL", &s.url, Color::Rgb(100, 150, 255)),
            info_line("Homepage", &s.homepage, Color::Rgb(100, 150, 255)),
            Line::from(""),
            Line::from(Span::styled(
                "Press i or Esc to close",
                Style::default().fg(Color::Rgb(80, 80, 110)),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No station selected",
            Style::default().fg(DIM_WHITE),
        ))]
    };

    let block = Block::default()
        .title(Span::styled(
            " Station Details ",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(CYAN))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup);
}