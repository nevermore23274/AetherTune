use crate::core::app::App;
use crate::ui::themes::Theme;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let themes = Theme::all();
    let total = themes.len();

    let popup_w: u16 = 40_u16.min(area.width);
    let popup_h: u16 = (total as u16 + 12).min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    f.render_widget(Clear, popup);

    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "🎨  Select Theme",
        Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("   Transparent bg: ", Style::default().fg(app.theme.text_muted)),
        Span::styled(
            if app.transparent_bg { "on" } else { "off" },
            Style::default()
                .fg(if app.transparent_bg { app.theme.positive } else { app.theme.text_muted })
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    for (i, theme) in themes.iter().enumerate() {
        let is_selected = i == app.theme_selected;
        let is_active = theme.name == app.theme.name;

        let (indicator, bg) = if is_selected {
            ("▸ ", Color::Rgb(40, 40, 70))
        } else {
            ("  ", Color::Reset)
        };

        let suffix = if is_active { " ●" } else { "" };

        let name_color = if is_active {
            app.theme.positive
        } else if is_selected {
            Color::White
        } else {
            Color::Rgb(160, 160, 180)
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, Style::default().fg(name_color).bg(bg)),
            // Color swatches showing the theme's accent, secondary, positive
            Span::styled("██", Style::default().fg(theme.accent).bg(bg)),
            Span::styled("██", Style::default().fg(theme.secondary).bg(bg)),
            Span::styled("██", Style::default().fg(theme.positive).bg(bg)),
            Span::styled(" ", Style::default().bg(bg)),
            Span::styled(
                format!("{}{}", theme.name, suffix),
                Style::default().fg(name_color).bg(bg),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{:─<width$}", "", width = 34),
        Style::default().fg(Color::Rgb(50, 50, 70)),
    )));
    lines.push(Line::from(vec![
        Span::styled("  ↑/↓", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" nav  ", Style::default().fg(app.theme.text_muted)),
        Span::styled("Enter", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" apply  ", Style::default().fg(app.theme.text_muted)),
        Span::styled("Esc", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" close", Style::default().fg(app.theme.text_muted)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  b", Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)),
        Span::styled(" toggle transparent bg", Style::default().fg(app.theme.text_muted)),
    ]));

    let block = Block::default()
        .title(Span::styled(
            " Theme ",
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