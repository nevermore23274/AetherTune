use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ── Color Palette ──────────────────────────────────────────────────
pub const CYAN: Color = Color::Rgb(0, 255, 255);
pub const MAGENTA: Color = Color::Rgb(200, 80, 255);
pub const NEON_GREEN: Color = Color::Rgb(57, 255, 20);
pub const DIM_WHITE: Color = Color::Rgb(160, 160, 180);
pub const DARK_BG: Color = Color::Rgb(15, 15, 25);
pub const PANEL_BG: Color = Color::Rgb(20, 20, 35);
pub const HIGHLIGHT_BG: Color = Color::Rgb(40, 40, 80);
pub const YELLOW: Color = Color::Rgb(255, 215, 0);
pub const ORANGE: Color = Color::Rgb(255, 140, 0);
pub const RED: Color = Color::Rgb(255, 60, 60);
pub const PEAK_COLOR: Color = Color::Rgb(255, 255, 255);

pub fn info_line(label: &str, value: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<8} ", label),
            Style::default().fg(Color::Rgb(100, 100, 130)),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

pub fn volume_line(volume: u32) -> Line<'static> {
    let filled = (volume as usize * 20) / 100;
    let empty = 20 - filled;
    let bar_color = if volume > 80 {
        RED
    } else if volume > 50 {
        YELLOW
    } else {
        NEON_GREEN
    };

    Line::from(vec![
        Span::styled(
            "  Vol ",
            Style::default().fg(Color::Rgb(100, 100, 130)),
        ),
        Span::styled("▐", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
        Span::styled("░".repeat(empty), Style::default().fg(Color::Rgb(40, 40, 60))),
        Span::styled("▌", Style::default().fg(Color::Rgb(60, 60, 80))),
        Span::styled(
            format!(" {}%", volume),
            Style::default().fg(bar_color),
        ),
    ])
}

pub fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {:<14}", key),
            Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(DIM_WHITE)),
    ])
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}