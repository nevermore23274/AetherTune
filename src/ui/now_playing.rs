use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let session_str = app.session_duration_str();
    let title_right = format!(" ⏱ {} ", session_str);

    let block = Block::default()
        .title(Span::styled(
            " Now Playing ",
            Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Draw session timer in top-right of the border
    if area.width > title_right.len() as u16 + 16 {
        let timer_x = area.x + area.width - title_right.len() as u16 - 1;
        let buf = f.buffer_mut();
        for (i, ch) in title_right.chars().enumerate() {
            let x = timer_x + i as u16;
            if x < area.x + area.width {
                buf.get_mut(x, area.y)
                    .set_char(ch)
                    .set_fg(Color::Rgb(100, 100, 140))
                    .set_bg(PANEL_BG);
            }
        }
    }

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    if let Some(np) = &app.now_playing {
        lines.push(Line::from(vec![
            Span::styled("♪ ", Style::default().fg(NEON_GREEN)),
            Span::styled(
                np.name.clone(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]));

        if let Some(ref title) = app.player.media_title {
            if title != &np.name && !title.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  ▸ ", Style::default().fg(Color::Rgb(80, 80, 120))),
                    Span::styled(
                        title.clone(),
                        Style::default().fg(YELLOW).add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(info_line("Genre", &np.genre, MAGENTA));
        lines.push(info_line("Country", &np.country, CYAN));
        lines.push(info_line("Bitrate", &format!("{} kbps", np.bitrate), YELLOW));
        lines.push(info_line("Codec", &np.codec, ORANGE));

        if !np.homepage.is_empty() {
            lines.push(info_line("Web", &np.homepage, Color::Rgb(100, 150, 255)));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "No station playing",
            Style::default().fg(Color::Rgb(80, 80, 100)),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Select a station and press Enter",
            Style::default().fg(Color::Rgb(60, 60, 80)),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(paragraph, inner);
}