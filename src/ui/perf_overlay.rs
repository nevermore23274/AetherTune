use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
    Frame,
    layout::Rect,
};

const SPARK_CHARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(55, 60, area);
    f.render_widget(Clear, popup);

    let summary = app.perf.summary();
    let avg = &summary.avg;
    let max = &summary.max;
    let tick_ms = app.tick_rate_ms;
    let tick_budget = tick_ms * 1000;
    let fps = if tick_ms > 0 { 1000 / tick_ms } else { 0 };

    let work_avg = avg.work_us();
    let usage_pct = if tick_budget > 0 {
        (work_avg as f64 / tick_budget as f64 * 100.0) as u64
    } else {
        0
    };

    let (usage_color, status_label) = if usage_pct > 80 {
        (RED, "OVER BUDGET")
    } else if usage_pct > 60 {
        (YELLOW, "TIGHT")
    } else if usage_pct > 30 {
        (NEON_GREEN, "OK")
    } else {
        (NEON_GREEN, "IDLE")
    };

    // Build sparkline from load history
    let load_history = app.perf.load_history_ordered();
    let sparkline = build_sparkline(&load_history, usage_color);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "  Tick ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                format!("{}ms", tick_ms),
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 70))),
            Span::styled(
                format!("~{} FPS", fps),
                Style::default().fg(CYAN),
            ),
            Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 70))),
            Span::styled("< > ", Style::default().fg(Color::Rgb(80, 80, 110))),
            Span::styled("adjust", Style::default().fg(Color::Rgb(60, 60, 90))),
        ]),
        Line::from(vec![
            Span::styled(
                "  Load ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                format!("{}%", usage_pct),
                Style::default().fg(usage_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} / {}µs)  ", work_avg, tick_budget),
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                status_label,
                Style::default().fg(usage_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    // Sparkline row
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        sparkline,
    ]));
    lines.push(Line::from(""));

    // Section header
    lines.push(section_header("Per-frame work (all frames)"));
    lines.push(column_header());
    lines.push(timing_line("Draw", avg.draw_us, max.draw_us));
    lines.push(timing_line("Key input", avg.event_handle_us, max.event_handle_us));
    lines.push(Line::from(""));

    // Tick-only section
    lines.push(section_header("Tick work (tick frames only)"));
    lines.push(column_header());
    lines.push(timing_line("IPC poll", summary.tick_avg_poll_us, summary.tick_max_poll_us));
    lines.push(timing_line("Visualizer", summary.tick_avg_vis_us, summary.tick_max_vis_us));
    lines.push(Line::from(""));

    // Totals
    lines.push(section_header("Totals"));
    lines.push(column_header());
    lines.push(timing_line("CPU work", work_avg, max.work_us()));
    lines.push(timing_line("Idle wait", avg.event_wait_us, max.event_wait_us));
    lines.push(timing_line("Frame", avg.total_us, max.total_us));
    lines.push(Line::from(""));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("IDLE", Style::default().fg(NEON_GREEN)),
        Span::styled(" <30%  ", Style::default().fg(Color::Rgb(60, 60, 90))),
        Span::styled("OK", Style::default().fg(NEON_GREEN)),
        Span::styled(" 30-60%  ", Style::default().fg(Color::Rgb(60, 60, 90))),
        Span::styled("TIGHT", Style::default().fg(YELLOW)),
        Span::styled(" 60-80%  ", Style::default().fg(Color::Rgb(60, 60, 90))),
        Span::styled("OVER", Style::default().fg(RED)),
        Span::styled(" >80%", Style::default().fg(Color::Rgb(60, 60, 90))),
    ]));
    lines.push(Line::from(Span::styled(
        "  ` close  │  < > tick rate  │  2s rolling window",
        Style::default().fg(Color::Rgb(60, 60, 90)),
    )));

    let block = Block::default()
        .title(Span::styled(
            " ⚡ Profiler ",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(YELLOW))
        .padding(Padding::new(1, 1, 1, 0))
        .style(Style::default().bg(Color::Rgb(10, 10, 20)));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, popup);
}

fn section_header(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("  ── {} ", label),
        Style::default().fg(CYAN),
    ))
}

fn column_header() -> Line<'static> {
    Line::from(Span::styled(
        "               avg µs    max µs",
        Style::default().fg(Color::Rgb(60, 60, 90)),
    ))
}

fn timing_line(label: &str, avg: u64, max: u64) -> Line<'static> {
    let color = if avg > 5000 {
        RED
    } else if avg > 2000 {
        YELLOW
    } else if avg > 500 {
        Color::Rgb(180, 200, 180)
    } else {
        NEON_GREEN
    };

    let max_color = if max > 10000 {
        RED
    } else if max > 5000 {
        YELLOW
    } else {
        Color::Rgb(100, 100, 130)
    };

    Line::from(vec![
        Span::styled(
            format!("  {:<12} ", label),
            Style::default().fg(Color::Rgb(140, 140, 160)),
        ),
        Span::styled(
            format!("{:>6}", avg),
            Style::default().fg(color),
        ),
        Span::styled(
            format!("    {:>6}", max),
            Style::default().fg(max_color),
        ),
    ])
}

fn build_sparkline(history: &[f64], color: Color) -> Span<'static> {
    let s: String = history
        .iter()
        .map(|&v| {
            let idx = (v * (SPARK_CHARS.len() - 1) as f64).round() as usize;
            SPARK_CHARS[idx.min(SPARK_CHARS.len() - 1)]
        })
        .collect();
    Span::styled(s, Style::default().fg(color))
}