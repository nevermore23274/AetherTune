use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let count = app.song_log.len();
    let title = if count > 0 {
        format!(" ♫ Song Log ({}) ", count)
    } else {
        " ♫ Song Log ".to_string()
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Rgb(180, 140, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 1 {
        return;
    }

    if app.song_log.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "Songs will appear here as they play…",
            Style::default().fg(Color::Rgb(60, 60, 80)),
        )));
        f.render_widget(empty_msg, inner);
        return;
    }

    let max_items = inner.height as usize;
    let avail = inner.width as usize;

    let lines: Vec<Line> = app
        .song_log
        .iter()
        .take(max_items)
        .enumerate()
        .map(|(i, entry)| {
            let is_current = i == 0;
            let time_style = Style::default().fg(Color::Rgb(80, 80, 110));
            let title_style = if is_current {
                Style::default().fg(YELLOW)
            } else {
                Style::default().fg(Color::Rgb(140, 140, 160))
            };
            let station_style = Style::default().fg(Color::Rgb(80, 80, 110));

            let indicator = if is_current { "▸ " } else { "  " };

            // Fixed-width prefix: "HH:MM " (6) + indicator (2) = 8 chars
            let prefix_len = entry.timestamp.len() + 1 + indicator.len();
            // Remaining space after prefix, split between title and station
            let remaining = avail.saturating_sub(prefix_len);

            // Station gets up to 30% of remaining space (min 10, max 20 chars)
            // plus 2 chars for the "  " separator
            let station_budget = (remaining * 30 / 100).clamp(10, 20);
            let title_budget = remaining.saturating_sub(station_budget + 2);

            let display_title = truncate_str(&entry.title, title_budget);
            let display_station = truncate_str(&entry.station, station_budget);

            Line::from(vec![
                Span::styled(format!("{} ", entry.timestamp), time_style),
                Span::styled(
                    indicator,
                    Style::default().fg(if is_current { NEON_GREEN } else { Color::Rgb(40, 40, 60) }),
                ),
                Span::styled(display_title, title_style),
                Span::styled(
                    format!("  {}", display_station),
                    station_style,
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}