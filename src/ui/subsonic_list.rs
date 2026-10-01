use crate::core::app::App;
use crate::subsonic::session::{BrowseTab, Entry};
use super::helpers::{format_duration, truncate_str};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

const DIM: Color = Color::Rgb(100, 100, 140);

/// Left panel while Subsonic is the active source: tab bar, then the
/// current level of the browser (artists → albums → songs, etc.).
pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let session = &app.subsonic;
    let border_color = app.theme.secondary;

    let title = match session.current_level() {
        Some(level) => {
            let path: Vec<&str> = session.stack.iter().map(|l| l.title.as_str()).collect();
            let more = if level.next_offset.is_some() { "+" } else { "" };
            let loading = if session.loading { " ⏳" } else { "" };
            let budget = (area.width as usize).saturating_sub(16);
            format!(
                " ♫ {} ({}{}){} ",
                truncate_start(&path.join(" › "), budget),
                level.entries.len(),
                more,
                loading
            )
        }
        None if session.loading => " ♫ Subsonic ⏳ Loading... ".to_string(),
        None => " ♫ Subsonic ".to_string(),
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg_panel));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 3 || inner.width < 10 {
        return;
    }

    if let Some(reason) = &session.setup_error {
        draw_message(f, app, inner, &[
            ("Subsonic isn't set up yet", app.theme.text_warn),
            ("", DIM),
            (reason, DIM),
            ("", DIM),
            ("Quit, then open Settings in the launch", DIM),
            ("menu to add your server.", DIM),
        ]);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tabs
            Constraint::Length(1), // error / hint
            Constraint::Min(1),    // list
        ])
        .split(inner);

    f.render_widget(Paragraph::new(tab_bar(app)), rows[0]);

    let status = if let Some(err) = &session.error {
        Line::from(Span::styled(
            truncate_str(&format!(" ⚠ {}", err), inner.width as usize),
            Style::default().fg(app.theme.text_error),
        ))
    } else if session.stack.len() > 1 {
        Line::from(Span::styled(
            format!(" {} back", crate::storage::config::keycode_to_string(app.keybindings.back.primary)),
            Style::default().fg(Color::Rgb(70, 70, 100)),
        ))
    } else {
        Line::from("")
    };
    f.render_widget(Paragraph::new(status), rows[1]);

    let Some(level) = session.current_level() else { return };
    if level.entries.is_empty() {
        let text = if session.loading { "" } else { "  Nothing here" };
        f.render_widget(
            Paragraph::new(Span::styled(text, Style::default().fg(DIM))),
            rows[2],
        );
        return;
    }

    let playing_id = session
        .active_queue(&app.player)
        .and_then(|queue| queue.current(&app.player))
        .map(|song| song.id.as_str());
    // borders(2) + highlight symbol(2) + marker(2)
    let name_budget = (inner.width as usize).saturating_sub(6);

    let items: Vec<ListItem> = level
        .entries
        .iter()
        .map(|entry| ListItem::new(entry_line(app, entry, playing_id, level.numbered, name_budget)))
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(app.theme.bg_highlight)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default();
    state.select(Some(level.selected));
    f.render_stateful_widget(list, rows[2], &mut state);
}

fn tab_bar(app: &App) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, tab) in [BrowseTab::Artists, BrowseTab::Albums, BrowseTab::Playlists].into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 100))));
        }
        let style = if tab == app.subsonic.tab {
            Style::default().fg(app.theme.positive).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(90, 90, 120))
        };
        spans.push(Span::styled(tab.title(), style));
    }
    spans.push(Span::styled(
        format!("   ({} to switch)", crate::storage::config::keycode_to_string(app.keybindings.cycle_panel.primary)),
        Style::default().fg(Color::Rgb(60, 60, 90)),
    ));
    Line::from(spans)
}

fn entry_line(app: &App, entry: &Entry, playing_id: Option<&str>, numbered: bool, budget: usize) -> Line<'static> {
    let name_style = Style::default().fg(app.theme.text_muted);
    let (marker, name, detail) = match entry {
        Entry::Artist(artist) => (
            "◆ ",
            artist.name.clone(),
            artist.album_count.map(|n| format!("{} album{}", n, if n == 1 { "" } else { "s" })),
        ),
        Entry::Album(album) => {
            let detail = match (&album.artist, album.year) {
                (Some(artist), Some(year)) => Some(format!("{} · {}", artist, year)),
                (Some(artist), None) => Some(artist.clone()),
                (None, Some(year)) => Some(year.to_string()),
                (None, None) => None,
            };
            ("◉ ", album.name.clone(), detail)
        }
        Entry::Playlist(playlist) => (
            "≡ ",
            playlist.name.clone(),
            playlist.song_count.map(|n| format!("{} song{}", n, if n == 1 { "" } else { "s" })),
        ),
        Entry::Song(song) => {
            let name = match song.track.filter(|_| numbered) {
                Some(n) => format!("{:>2}. {}", n, song.title),
                None => song.title.clone(),
            };
            // Duration first so it survives truncation
            let mut detail: Vec<String> = Vec::new();
            if let Some(secs) = song.duration {
                detail.push(format_duration(secs as f64));
            }
            if let Some(artist) = &song.artist {
                detail.push(artist.clone());
            }
            let marker = if playing_id == Some(song.id.as_str()) { "♪ " } else { "  " };
            (marker, name, (!detail.is_empty()).then(|| detail.join(" · ")))
        }
    };

    // Give the detail column up to half the width before truncating the name
    let detail_width = detail.as_ref().map_or(0, |d| d.chars().count() + 3);
    let name_budget = budget.saturating_sub(detail_width).max(budget / 2);
    let name_width = name.chars().count().min(name_budget);
    let mut spans = vec![
        Span::styled(marker, Style::default().fg(app.theme.positive)),
        Span::styled(truncate_str(&name, name_budget), name_style),
    ];
    if let Some(detail) = detail {
        let room = budget.saturating_sub(name_width + 3);
        if room > 3 {
            spans.push(Span::styled(
                format!(" │ {}", truncate_str(&detail, room)),
                Style::default().fg(DIM),
            ));
        }
    }
    Line::from(spans)
}

fn draw_message(f: &mut Frame, app: &App, area: Rect, lines: &[(&str, Color)]) {
    let lines: Vec<Line> = std::iter::once(Line::from(""))
        .chain(lines.iter().map(|(text, color)| {
            let style = if *color == app.theme.text_warn {
                Style::default().fg(*color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(*color)
            };
            Line::from(Span::styled(format!(" {}", text), style))
        }))
        .collect();
    f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), area);
}

/// Keeps the end of a breadcrumb path (the part you're in) when too long.
fn truncate_start(text: &str, max: usize) -> String {
    let len = text.chars().count();
    if len <= max || max < 2 {
        return text.to_string();
    }
    let tail: String = text.chars().skip(len - max + 1).collect();
    format!("…{}", tail)
}
