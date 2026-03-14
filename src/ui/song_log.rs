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

            let max_title_len = (inner.width as usize).saturating_sub(18);
            let display_title = truncate_str(&entry.title, max_title_len);

            Line::from(vec![
                Span::styled(format!("{} ", entry.timestamp), time_style),
                Span::styled(
                    if is_current { "▸ " } else { "  " },
                    Style::default().fg(if is_current { NEON_GREEN } else { Color::Rgb(40, 40, 60) }),
                ),
                Span::styled(display_title, title_style),
                Span::styled(
                    format!("  {}", truncate_str(&entry.station, 15)),
                    station_style,
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}