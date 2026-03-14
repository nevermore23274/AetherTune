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
    let block = Block::default()
        .title(Span::styled(
            " Stream ",
            Style::default()
                .fg(Color::Rgb(100, 200, 180))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 1 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Volume is always shown
    lines.push(volume_compact(app.volume));

    if !app.player.is_playing() {
        // Only show as many lines as we have room
        let visible: Vec<Line> = lines.into_iter().take(inner.height as usize).collect();
        f.render_widget(Paragraph::new(visible), inner);
        return;
    }

    let si = &app.player.stream_info;

    // Connection uptime
    let uptime = si.uptime_str();
    lines.push(compact_line("Uptime", &uptime, NEON_GREEN));

    // Audio bitrate (actual from mpv vs advertised)
    if si.audio_bitrate > 0.0 {
        let actual_kbps = si.audio_bitrate / 1000.0;
        let advertised = app
            .now_playing
            .as_ref()
            .map(|np| np.bitrate)
            .unwrap_or(0);

        let bitrate_str = if advertised > 0 {
            format!("{:.0} / {} kbps", actual_kbps, advertised)
        } else {
            format!("{:.0} kbps", actual_kbps)
        };

        // Color based on how close actual is to advertised
        let color = if advertised > 0 {
            let ratio = actual_kbps / advertised as f64;
            if ratio > 0.85 {
                NEON_GREEN
            } else if ratio > 0.5 {
                YELLOW
            } else {
                RED
            }
        } else {
            CYAN
        };
        lines.push(compact_line("Bitrate", &bitrate_str, color));
    } else {
        lines.push(compact_line("Bitrate", "—", Color::Rgb(60, 60, 80)));
    }

    // Codec + sample rate + channels on one line
    let codec = if si.audio_codec.is_empty() {
        app.now_playing
            .as_ref()
            .map(|np| np.codec.clone())
            .unwrap_or_default()
    } else {
        si.audio_codec.clone()
    };

    let audio_fmt = if si.sample_rate > 0 && si.channels > 0 {
        let ch_str = match si.channels {
            1 => "mono",
            2 => "stereo",
            _ => "multi",
        };
        format!("{} {}Hz {}", codec, si.sample_rate, ch_str)
    } else if !codec.is_empty() {
        codec
    } else {
        "—".to_string()
    };
    lines.push(compact_line("Format", &audio_fmt, CYAN));

    // Buffer health
    if si.cache_duration > 0.0 {
        let buf_str = format!("{:.1}s", si.cache_duration);
        let color = if si.cache_duration > 5.0 {
            NEON_GREEN
        } else if si.cache_duration > 2.0 {
            YELLOW
        } else {
            RED
        };

        // Simple visual bar for buffer
        let bar_len = ((si.cache_duration / 15.0) * 8.0).min(8.0) as usize;
        let bar = format!("{}{} {}", "█".repeat(bar_len), "░".repeat(8 - bar_len), buf_str);
        lines.push(compact_line("Buffer", &bar, color));
    } else {
        lines.push(compact_line("Buffer", "—", Color::Rgb(60, 60, 80)));
    }

    // Only show as many lines as we have room
    let visible: Vec<Line> = lines.into_iter().take(inner.height as usize).collect();
    f.render_widget(Paragraph::new(visible), inner);
}

fn compact_line(label: &str, value: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<7} ", label),
            Style::default().fg(Color::Rgb(100, 100, 130)),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn volume_compact(volume: u32) -> Line<'static> {
    let filled = (volume as usize * 10) / 100;
    let empty = 10 - filled;
    let bar_color = if volume > 80 {
        RED
    } else if volume > 50 {
        YELLOW
    } else {
        NEON_GREEN
    };

    Line::from(vec![
        Span::styled(
            "Vol     ",
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