use crate::app::{ActivePanel, App};
use crate::storage::config::binding_display;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
    layout::Rect,
};

pub fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(60, 70, area);
    f.render_widget(Clear, popup);

    let kb = &app.keybindings;
    let help_text = vec![
        Line::from(Span::styled(
            "⌨  Keybindings",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line(&binding_display(&kb.navigate_up), "Navigate up"),
        help_line(&binding_display(&kb.navigate_down), "Navigate down"),
        help_line(&binding_display(&kb.play), "Play selected station"),
        help_line(&binding_display(&kb.stop), "Stop playback"),
        help_line(&binding_display(&kb.volume_up), "Volume up"),
        help_line(&binding_display(&kb.volume_down), "Volume down"),
        help_line(&binding_display(&kb.search), "Search stations"),
        help_line(&binding_display(&kb.toggle_favorite), "Toggle favorite"),
        help_line(&binding_display(&kb.station_detail), "Station details"),
        help_line(&binding_display(&kb.load_more), "Load more stations"),
        help_line(&binding_display(&kb.cycle_panel), "Cycle panel"),
        help_line(&binding_display(&kb.genre_next), "Next genre"),
        help_line(&binding_display(&kb.genre_prev), "Previous genre"),
        help_line(&binding_display(&kb.help), "Toggle this help"),
        help_line(&binding_display(&kb.perf_toggle), "Performance profiler"),
        help_line(&binding_display(&kb.perf_tick_slower), "Tick rate slower (profiler)"),
        help_line(&binding_display(&kb.perf_tick_faster), "Tick rate faster (profiler)"),
        help_line(&binding_display(&kb.settings), "Keybinding settings"),
        help_line(&binding_display(&kb.quit), "Quit"),
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