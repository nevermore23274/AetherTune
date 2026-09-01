use crate::core::app::App;
use crate::core::types::ActivePanel;
use super::helpers::truncate_str;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    // Calculate dynamic name width: area minus borders (2), highlight symbol (2),
    // prefix markers (4)
    let name_budget = (area.width as usize).saturating_sub(10);

    // Build the set of favorite URLs once per draw() call rather than
    // linear-scanning app.favorites.entries for every station/history row
    // (previously O(stations * favorites) string comparisons every single
    // frame — this made it O(stations) hash lookups instead).
    let favorite_urls: std::collections::HashSet<&str> =
        app.favorites.entries.iter().map(|f| f.url.as_str()).collect();

    let (title, items, selected) = match app.active_panel {
        ActivePanel::Stations => {
            let items: Vec<ListItem> = app
                .stations
                .iter()
                .map(|s| {
                    let fav_marker = if favorite_urls.contains(s.url.as_str()) { "★ " } else { "  " };
                    let playing_marker = app
                        .now_playing
                        .as_ref()
                        .map(|np| np.url == s.url)
                        .unwrap_or(false);
                    let prefix = if playing_marker { "♪ " } else { "  " };

                    let line = Line::from(vec![
                        Span::styled(fav_marker, Style::default().fg(app.theme.text_warn)),
                        Span::styled(prefix, Style::default().fg(app.theme.positive)),
                        Span::styled(
                            truncate_str(&s.name, name_budget),
                            Style::default().fg(app.theme.text_muted),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            (
                if app.is_loading {
                    " Stations ⏳ Loading... ".to_string()
                } else if app.has_more {
                    format!(" Stations ({}+) [n: more] ", app.stations.len())
                } else {
                    format!(" Stations ({}) ", app.stations.len())
                },
                items,
                app.selected_index,
            )
        }
        ActivePanel::Favorites => {
            let items: Vec<ListItem> = app
                .favorites
                .entries
                .iter()
                .map(|fav| {
                    let playing = app
                        .now_playing
                        .as_ref()
                        .map(|np| np.url == fav.url)
                        .unwrap_or(false);
                    let prefix = if playing { "♪ " } else { "  " };

                    let line = Line::from(vec![
                        Span::styled("★ ", Style::default().fg(app.theme.text_warn)),
                        Span::styled(prefix, Style::default().fg(app.theme.positive)),
                        Span::styled(
                            truncate_str(&fav.name, name_budget),
                            Style::default().fg(app.theme.text_muted),
                        ),
                        Span::styled(
                            format!(" │ {}", truncate_str(&fav.genre, 15)),
                            Style::default().fg(Color::Rgb(100, 100, 140)),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            (
                format!(" ★ Favorites ({}) ", app.favorites.entries.len()),
                items,
                app.fav_selected_index,
            )
        }
        ActivePanel::History => {
            let items: Vec<ListItem> = app
                .history
                .entries
                .iter()
                .map(|h| {
                    let fav_marker = if favorite_urls.contains(h.url.as_str()) { "★ " } else { "  " };
                    let time_str = &h.played_at;
                    let line = Line::from(vec![
                        Span::styled(fav_marker, Style::default().fg(app.theme.text_warn)),
                        Span::styled(
                            truncate_str(&h.name, name_budget),
                            Style::default().fg(app.theme.text_muted),
                        ),
                        Span::styled(
                            format!(" │ {}", time_str),
                            Style::default().fg(Color::Rgb(100, 100, 140)),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            (
                format!(" ⏱ History ({}) ", app.history.entries.len()),
                items,
                app.hist_selected_index,
            )
        }
    };

    let border_color = match app.active_panel {
        ActivePanel::Stations => app.theme.accent,
        ActivePanel::Favorites => app.theme.text_warn,
        ActivePanel::History => app.theme.secondary,
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

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(app.theme.bg_highlight)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
}