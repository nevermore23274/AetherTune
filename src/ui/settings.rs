use crate::app::App;
use crate::storage::config::keycode_to_string;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
    layout::Rect,
};

const SELECTED_BG: Color = Color::Rgb(30, 30, 60);
const AWAITING_BG: Color = Color::Rgb(60, 30, 20);

// Column widths (character counts, must be consistent between header and rows)
const COL_ACTION: usize = 24; // includes 2-char indicator prefix
const COL_PRIMARY: usize = 8;
// Alt column gets the remainder — no pad needed, it's the last column

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let popup_w: u16 = 48;
    let popup_h: u16 = 33;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w.min(area.width), popup_h.min(area.height));
    f.render_widget(Clear, popup);

    let actions = app.keybindings.all_actions();
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "⚙  Keybinding Settings",
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Column headers — use exact same widths as data rows
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:<width$}", "  Action", width = COL_ACTION),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<width$}", "Primary", width = COL_PRIMARY),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Alt".to_string(),
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(Span::styled(
        format!("{:─<width$}", "", width = COL_ACTION + COL_PRIMARY + 5),
        Style::default().fg(Color::Rgb(50, 50, 70)),
    )));

    for (i, (_key, label, binding)) in actions.iter().enumerate() {
        let is_selected = i == app.settings_selected;
        let is_awaiting = app.settings_awaiting_key.map_or(false, |(idx, _)| idx == i);
        let awaiting_slot = app.settings_awaiting_key
            .and_then(|(idx, is_alt)| if idx == i { Some(is_alt) } else { None });

        let row_bg = if is_awaiting {
            AWAITING_BG
        } else if is_selected {
            SELECTED_BG
        } else {
            Color::Reset
        };

        let indicator = if is_selected { "▸ " } else { "  " };
        let label_color = if is_selected { Color::White } else { DIM_WHITE };

        // Build the action cell: indicator + label, padded to COL_ACTION total
        let action_text = format!("{}{}", indicator, label);
        let action_cell = format!("{:<width$}", action_text, width = COL_ACTION);

        let primary_str = keycode_to_string(binding.primary);
        let alt_str = binding.alt.map_or("—".to_string(), keycode_to_string);

        let primary_style = if awaiting_slot == Some(false) {
            Style::default().fg(RED).add_modifier(Modifier::SLOW_BLINK | Modifier::BOLD).bg(row_bg)
        } else {
            Style::default().fg(NEON_GREEN).bg(row_bg)
        };

        let alt_style = if awaiting_slot == Some(true) {
            Style::default().fg(RED).add_modifier(Modifier::SLOW_BLINK | Modifier::BOLD).bg(row_bg)
        } else {
            Style::default().fg(ORANGE).bg(row_bg)
        };

        let primary_display = if awaiting_slot == Some(false) {
            "▓▓▓".to_string()
        } else {
            primary_str
        };

        let alt_display = if awaiting_slot == Some(true) {
            "▓▓▓".to_string()
        } else {
            alt_str
        };

        lines.push(Line::from(vec![
            Span::styled(
                action_cell,
                Style::default().fg(label_color).bg(row_bg),
            ),
            Span::styled(
                format!("{:<width$}", primary_display, width = COL_PRIMARY),
                primary_style,
            ),
            Span::styled(
                alt_display,
                alt_style,
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{:─<width$}", "", width = COL_ACTION + COL_PRIMARY + 5),
        Style::default().fg(Color::Rgb(50, 50, 70)),
    )));

    // Instructions
    if app.settings_awaiting_key.is_some() {
        lines.push(Line::from(Span::styled(
            "  Press any key to assign • Esc cancel",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ↑/↓", Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" nav ", Style::default().fg(DIM_WHITE)),
            Span::styled("Enter", Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" primary ", Style::default().fg(DIM_WHITE)),
            Span::styled("a", Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" alt ", Style::default().fg(DIM_WHITE)),
            Span::styled("d", Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" clear", Style::default().fg(DIM_WHITE)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  r", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(" reset ", Style::default().fg(DIM_WHITE)),
            Span::styled("R", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(" reset all ", Style::default().fg(DIM_WHITE)),
            Span::styled("Esc/S", Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(" close", Style::default().fg(DIM_WHITE)),
        ]));
    }

    let block = Block::default()
        .title(Span::styled(
            " Settings ",
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MAGENTA))
        .padding(Padding::new(1, 1, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup);
}