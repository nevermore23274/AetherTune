use crate::core::app::App;
use crate::core::types::ActivePanel;
use crate::storage::config::binding_display;
use super::helpers::{help_line_themed, info_line, centered_rect};
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
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line_themed(&binding_display(&kb.navigate_up), "Navigate up", &app.theme),
        help_line_themed(&binding_display(&kb.navigate_down), "Navigate down", &app.theme),
        help_line_themed(&binding_display(&kb.play), "Play selected station", &app.theme),
        help_line_themed(&binding_display(&kb.stop), "Stop playback", &app.theme),
        help_line_themed(&binding_display(&kb.volume_up), "Volume up", &app.theme),
        help_line_themed(&binding_display(&kb.volume_down), "Volume down", &app.theme),
        help_line_themed(&binding_display(&kb.search), "Search stations", &app.theme),
        help_line_themed(&binding_display(&kb.toggle_favorite), "Toggle favorite", &app.theme),
        help_line_themed(&binding_display(&kb.station_detail), "Station details", &app.theme),
        help_line_themed(&binding_display(&kb.load_more), "Load more stations", &app.theme),
        help_line_themed(&binding_display(&kb.cycle_panel), "Cycle panel", &app.theme),
        help_line_themed(&binding_display(&kb.genre_next), "Next genre", &app.theme),
        help_line_themed(&binding_display(&kb.genre_prev), "Previous genre", &app.theme),
        help_line_themed(&binding_display(&kb.genre_picker), "Genre picker", &app.theme),
        help_line_themed(&binding_display(&kb.help), "Toggle this help", &app.theme),
        help_line_themed(&binding_display(&kb.perf_toggle), "Performance profiler", &app.theme),
        help_line_themed(&binding_display(&kb.perf_tick_slower), "Tick rate slower (profiler)", &app.theme),
        help_line_themed(&binding_display(&kb.perf_tick_faster), "Tick rate faster (profiler)", &app.theme),
        help_line_themed(&binding_display(&kb.settings), "Keybinding settings", &app.theme),
        help_line_themed(&binding_display(&kb.theme_picker), "Theme picker", &app.theme),
        help_line_themed(&binding_display(&kb.visualizer_toggle), "Toggle visualizer", &app.theme),
        help_line_themed(&binding_display(&kb.quit), "Quit", &app.theme),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? or Esc to close",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " Help ",
            Style::default().fg(app.theme.text_warn).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.text_warn))
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
            info_line("Genre", &s.tags, app.theme.secondary),
            info_line("Country", &s.country, app.theme.accent),
            info_line("Bitrate", &format!("{} kbps", s.bitrate), app.theme.text_warn),
            info_line("Codec", &s.codec, app.theme.text_warm),
            info_line("Votes", &s.votes.to_string(), app.theme.positive),
            info_line("Favorite", fav, app.theme.text_warn),
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
            Style::default().fg(app.theme.text_muted),
        ))]
    };

    let block = Block::default()
        .title(Span::styled(
            " Station Details ",
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup);
}