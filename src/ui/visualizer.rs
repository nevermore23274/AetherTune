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
    // Divide the panel evenly among bars. Gap is 1 column when bars are
    // narrow (≤3 cols), but grows proportionally so the ratio of bar-to-gap
    // stays roughly the same on both 1080p and 4K.
    let min_stride = (inner.width / num_bars as u16).max(2);
    let gap = (min_stride / 4).max(1).min(min_stride - 1);
    let bar_w = min_stride - gap;
    let stride = bar_w + gap;
    let total_used = stride * num_bars as u16 - gap;
    let x_offset = inner.x + (inner.width.saturating_sub(total_used)) / 2;

    for i in 0..num_bars {
        // Scale logical bar/peak heights (0..logical_max) to display rows (0..display_h)
        let bar_rows = (vis.bars[i] as u32 * display_h as u32 / logical_max as u32) as u16;
        let peak_row = (vis.peaks[i] as u32 * display_h as u32 / logical_max as u32) as u16;
        let bar_rows = bar_rows.min(display_h);
        let peak_row = peak_row.min(display_h);
        let bar_x = x_offset + (i as u16) * stride;

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