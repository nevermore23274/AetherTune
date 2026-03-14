use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(55, 50, area);
    f.render_widget(Clear, popup);

    let avg = app.perf.summary();
    let max = app.perf.max();
    let tick_ms = app.tick_rate_ms;
    let tick_budget = tick_ms * 1000; // budget in µs
    let fps = if tick_ms > 0 { 1000 / tick_ms } else { 0 };

    // Budget based on actual CPU work, not idle wait
    let work_avg = avg.work_us();
    let usage_pct = if tick_budget > 0 {
        (work_avg as f64 / tick_budget as f64 * 100.0) as u64
    } else {
        0
    };

    let usage_color = if usage_pct > 80 {
        RED
    } else if usage_pct > 50 {
        YELLOW
    } else {
        NEON_GREEN
    };

    let lines = vec![
        Line::from(Span::styled(
            "Performance Profiler",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        stat_line(
            "Tick rate",
            &format!("{}ms (~{} FPS)  [< > to adjust]", tick_ms, fps),
            CYAN,
        ),
        stat_line(
            "CPU load",
            &format!("{}% of budget ({}/{}µs)", usage_pct, work_avg, tick_budget),
            usage_color,
        ),
        Line::from(""),
        Line::from(Span::styled(
            "  CPU work        avg µs    max µs",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
        timing_line("Draw", avg.draw_us, max.draw_us),
        timing_line("Key handle", avg.event_handle_us, max.event_handle_us),
        timing_line("IPC poll", avg.poll_us, max.poll_us),
        timing_line("Visualizer", avg.vis_us, max.vis_us),
        timing_line("Work total", work_avg, max.work_us()),
        Line::from(""),
        Line::from(Span::styled(
            "  Idle",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
        timing_line("Event wait", avg.event_wait_us, max.event_wait_us),
        Line::from(""),
        Line::from(Span::styled(
            "  Wall total",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
        timing_line("Frame", avg.total_us, max.total_us),
        Line::from(""),
        Line::from(Span::styled(
            "` to close  │  < > adjust tick rate",
            Style::default().fg(Color::Rgb(80, 80, 110)),
        )),
    ];

    let block = Block::default()
        .title(Span::styled(
            " ⚡ Perf ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(YELLOW))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup);
}

fn stat_line(label: &str, value: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {:<10} ", label),
            Style::default().fg(Color::Rgb(100, 100, 130)),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn timing_line(label: &str, avg: u64, max: u64) -> Line<'static> {
    let color = if avg > 5000 {
        RED
    } else if avg > 2000 {
        YELLOW
    } else {
        NEON_GREEN
    };

    Line::from(vec![
        Span::styled(
            format!("  {:<10} ", label),
            Style::default().fg(Color::Rgb(100, 100, 130)),
        ),
        Span::styled(
            format!("{:>6}", avg),
            Style::default().fg(color),
        ),
        Span::styled(
            format!("    {:>6}", max),
            Style::default().fg(Color::Rgb(140, 140, 160)),
        ),
    ])
}