use crate::app::App;
use super::helpers::*;
use ratatui::{
    style::{Color, Style},
    widgets::{Block, BorderType, Borders},
    Frame,
    layout::Rect,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .style(Style::default().bg(PANEL_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let vis = &app.visualizer;
    let num_bars = vis.num_bars().min(inner.width as usize / 2);
    let max_h = vis.max_height().min(inner.height);

    let x_offset = inner.x + (inner.width.saturating_sub(num_bars as u16 * 2)) / 2;

    for i in 0..num_bars {
        let bar_h = vis.bars[i].min(max_h);
        let peak_h = vis.peaks[i].min(max_h);

        for row in 0..max_h {
            let y = inner.y + (max_h - 1 - row);
            let x = x_offset + (i as u16) * 2;

            if x >= inner.x + inner.width {
                break;
            }

            if y >= inner.y + inner.height {
                continue;
            }

            if row == peak_h && peak_h > bar_h {
                let buf = f.buffer_mut();
                buf.get_mut(x, y)
                    .set_char('─')
                    .set_fg(PEAK_COLOR);
            } else if row < bar_h && bar_h > 0 {
                let frac = row as f32 / bar_h as f32;
                let color = if frac < 0.5 {
                    let t = frac / 0.5;
                    Color::Rgb(
                        (0.0 + t * 220.0) as u8,
                        (220.0 + t * 0.0) as u8,
                        (180.0 * (1.0 - t)) as u8,
                    )
                } else {
                    let t = (frac - 0.5) / 0.5;
                    Color::Rgb(
                        (220.0 + t * 35.0) as u8,
                        (220.0 * (1.0 - t)) as u8,
                        0,
                    )
                };

                let buf = f.buffer_mut();
                buf.get_mut(x, y)
                    .set_char('█')
                    .set_fg(color);
            }
        }
    }
}