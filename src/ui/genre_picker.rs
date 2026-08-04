use crate::core::app::App;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
    layout::Rect,
};

const SELECTED_BG: Color = Color::Rgb(30, 30, 60);
const ACTIVE_BG: Color = Color::Rgb(20, 50, 30);

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    // Fixed popup size — genres scroll within it
    let popup_w: u16 = 34_u16.min(area.width);
    let popup_h: u16 = 30_u16.min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    f.render_widget(Clear, popup);

    // Inner height = popup_h - 2 (border) - 2 (padding) = usable rows
    // Header takes 2 lines (title + blank), footer takes 3 (blank + separator + instructions)
    // So visible genre slots = inner_h - 5
    let inner_h = popup_h.saturating_sub(4) as usize; // border + padding
    let visible_slots = inner_h.saturating_sub(5);
    let total = app.categories.len();

    // Scroll offset: keep selected item visible
    let scroll_offset = if app.genre_selected < visible_slots / 2 {
        0
    } else if app.genre_selected + visible_slots / 2 >= total {
        total.saturating_sub(visible_slots)
    } else {
        app.genre_selected.saturating_sub(visible_slots / 2)
    };

    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "♫  Select Genre",
        Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Show scroll indicator if not at top
    if scroll_offset > 0 {
        lines.push(Line::from(Span::styled(
            "  ↑ more",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )));
    }

    let end = (scroll_offset + visible_slots).min(total);
    for i in scroll_offset..end {
        let genre = app.categories[i];
        let is_selected = i == app.genre_selected;
        let is_active = i == app.category_index;

        let (indicator, bg, fg) = if is_selected && is_active {
            ("▸ ", ACTIVE_BG, app.theme.positive)
        } else if is_selected {
            ("▸ ", SELECTED_BG, Color::White)
        } else if is_active {
            ("  ", ACTIVE_BG, app.theme.positive)
        } else {
            ("  ", Color::Reset, app.theme.text_muted)
        };

        let suffix = if is_active { " ●" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{}{}{}", indicator, genre, suffix),
                Style::default().fg(fg).bg(bg),
            ),
        ]));
    }

    // Show scroll indicator if not at bottom
    if end < total {
        lines.push(Line::from(Span::styled(
            "  ↓ more",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{:─<width$}", "", width = 28),
        Style::default().fg(Color::Rgb(50, 50, 70)),
    )));
    lines.push(Line::from(vec![
        Span::styled("  ↑/↓", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" nav  ", Style::default().fg(app.theme.text_muted)),
        Span::styled("Enter", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" select  ", Style::default().fg(app.theme.text_muted)),
        Span::styled("Esc", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" close", Style::default().fg(app.theme.text_muted)),
    ]));

    let block = Block::default()
        .title(Span::styled(
            " Genre ",
            Style::default().fg(app.theme.secondary).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.secondary))
        .padding(Padding::new(1, 1, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup);
}