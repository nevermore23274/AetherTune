use crate::core::app::App;
use super::helpers::{format_duration, info_line, truncate_str};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Now Playing contents while a Subsonic queue is playing, or None to fall
/// back to the radio view. Drawn inside now_playing.rs's panel.
pub fn lines(app: &App, width: u16) -> Option<Vec<Line<'static>>> {
    let queue = app.subsonic.active_queue(&app.player)?;
    let width = width as usize;
    let playback = &app.player.playback;

    let Some(song) = queue.current(&app.player) else {
        return Some(vec![
            Line::from(""),
            Line::from(Span::styled("Starting playback…", Style::default().fg(Color::Rgb(100, 100, 140)))),
        ]);
    };

    let mut lines = vec![Line::from(vec![
        Span::styled("♪ ", Style::default().fg(app.theme.positive)),
        Span::styled(
            truncate_str(&song.title, width.saturating_sub(2)),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ])];
    if let Some(artist) = &song.artist {
        lines.push(Line::from(Span::styled(
            format!("  {}", truncate_str(artist, width.saturating_sub(2))),
            Style::default().fg(app.theme.accent),
        )));
    }
    if let Some(album) = &song.album {
        lines.push(Line::from(Span::styled(
            format!("  {}", truncate_str(album, width.saturating_sub(2))),
            Style::default().fg(app.theme.secondary).add_modifier(Modifier::ITALIC),
        )));
    }
    lines.push(Line::from(""));

    let elapsed = playback.time_pos.unwrap_or(0.0);
    let duration = queue.current_duration(&app.player);
    lines.push(progress_line(app, elapsed, duration, width));

    let index = queue.current_index(&app.player).unwrap_or(0);
    let state = if playback.paused { "❚❚ Paused" } else { "▶ Playing" };
    lines.push(info_line(
        "Status",
        &format!("{} · track {} of {}", state, index + 1, queue.songs().len()),
        if playback.paused { app.theme.text_warn } else { app.theme.positive },
    ));

    let format = match (&song.suffix, song.bit_rate) {
        (Some(suffix), Some(kbps)) => format!("{} · {} kbps", suffix.to_uppercase(), kbps),
        (Some(suffix), None) => suffix.to_uppercase(),
        (None, Some(kbps)) => format!("{} kbps", kbps),
        (None, None) => String::new(),
    };
    if !format.is_empty() {
        lines.push(info_line("Format", &format, app.theme.text_warm));
    }

    Some(lines)
}

/// "  1:23 ━━━━━━━━──────── 4:56"
fn progress_line(app: &App, elapsed: f64, duration: Option<f64>, width: usize) -> Line<'static> {
    let left = format_duration(elapsed);
    let right = duration.map(format_duration).unwrap_or_else(|| "--:--".to_string());
    let bar_width = width.saturating_sub(left.len() + right.len() + 4).max(4);
    let fraction = match duration {
        Some(d) if d > 0.0 => (elapsed / d).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let filled = (fraction * bar_width as f64).round() as usize;

    Line::from(vec![
        Span::styled(format!("  {} ", left), Style::default().fg(Color::Rgb(150, 150, 180))),
        Span::styled("━".repeat(filled), Style::default().fg(app.theme.positive)),
        Span::styled("─".repeat(bar_width - filled), Style::default().fg(Color::Rgb(50, 50, 75))),
        Span::styled(format!(" {}", right), Style::default().fg(Color::Rgb(150, 150, 180))),
    ])
}
