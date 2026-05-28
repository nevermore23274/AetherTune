use crate::app::App;

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
        .style(Style::default().bg(app.theme.bg_panel));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let vis = &app.visualizer;
    let num_bars = vis.num_bars().min(inner.width as usize);
    let logical_max = vis.max_height(); // The 0..12 range from the visualizer logic
    let display_h = inner.height;       // Actual rows available in the panel

    if num_bars == 0 || display_h == 0 {
        return;
    }

    // ── Horizontal sizing ──
    // Position each bar proportionally across the full panel width so all
    // space is used, regardless of how num_bars divides into inner.width.
    // Each bar's left edge is at: inner.x + i * inner.width / num_bars
    // Gap is 1 column (or ~20% of per-bar width, whichever is larger).
    let per_bar_width = inner.width / num_bars as u16;
    let gap = if per_bar_width <= 2 { 1 } else { (per_bar_width + 3) / 5 };
    let gap = gap.min(per_bar_width.saturating_sub(1)).max(1);

    for i in 0..num_bars {
        let bar_x = inner.x + (i as u32 * inner.width as u32 / num_bars as u32) as u16;
        let next_x = inner.x + ((i as u32 + 1) * inner.width as u32 / num_bars as u32) as u16;
        let bar_w = (next_x - bar_x).saturating_sub(gap).max(1);

        // Scale logical bar/peak heights (0..logical_max) to display rows (0..display_h)
        let bar_rows = (vis.bars[i] as u32 * display_h as u32 / logical_max as u32) as u16;
        let peak_row = (vis.peaks[i] as u32 * display_h as u32 / logical_max as u32) as u16;
        let bar_rows = bar_rows.min(display_h);
        let peak_row = peak_row.min(display_h);

        for row in 0..display_h {
            let y = inner.y + (display_h - 1 - row);

            if row == peak_row && peak_row > bar_rows {
                let buf = f.buffer_mut();
                for col in 0..bar_w {
                    let x = bar_x + col;
                    if x >= inner.x + inner.width {
                        break;
                    }
                    buf.get_mut(x, y)
                        .set_char('─')
                        .set_fg(app.theme.peak);
                }
            } else if row < bar_rows && bar_rows > 0 {
                // Color based on absolute position in the panel, not relative
                // to bar height. Bottom rows are always cool (cyan/green),
                // top rows are warm (orange/red). Short bars stay cool.
                let frac = row as f32 / display_h as f32;
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
                for col in 0..bar_w {
                    let x = bar_x + col;
                    if x >= inner.x + inner.width {
                        break;
                    }
                    buf.get_mut(x, y)
                        .set_char('█')
                        .set_fg(color);
                }
            }
        }
    }
}