use crossterm::event::{self, Event};
use ratatui::{
    backend::CrosstermBackend,
    style::Color,
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

// ── CRT power-off timing (milliseconds) ─────────────────────────────
const PHASE1_COLLAPSE_MS: u64 = 500;   // Screen collapses vertically to a horizontal line
const PHASE2_SQUEEZE_MS: u64 = 800;    // Line squeezes horizontally to a bright dot
const PHASE3_DOT_HOLD_MS: u64 = 1200;  // Dot lingers with phosphor glow
const PHASE4_FADE_MS: u64 = 1600;      // Dot fades to black
const TOTAL_MS: u64 = PHASE4_FADE_MS;

const BG: Color = Color::Rgb(5, 5, 10);

/// Play the CRT power-off animation, then return.
///
/// Runs its own rendering loop at ~60 fps.  Any keypress skips straight
/// to the end so the user is never trapped.
pub fn play(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let start = Instant::now();

    loop {
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed >= TOTAL_MS {
            break;
        }

        terminal.draw(|f| {
            let area = f.size();
            let w = area.width as f64;
            let h = area.height as f64;
            let cx = area.left() + area.width / 2;
            let cy = area.top() + area.height / 2;

            // Fill background
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    let cell = f.buffer_mut().get_mut(x, y);
                    cell.set_char(' ').set_fg(BG).set_bg(BG);
                }
            }

            if elapsed < PHASE1_COLLAPSE_MS {
                // ── Phase 1: Vertical collapse ──────────────────────
                // The visible area shrinks vertically toward the centre line.
                // We simulate this by drawing a bright horizontal band that
                // narrows over time, with static/noise inside it.
                let t = elapsed as f64 / PHASE1_COLLAPSE_MS as f64;
                let half = ((1.0 - t) * h / 2.0).max(0.0) as u16;

                let top = cy.saturating_sub(half).max(area.top());
                let bot = (cy + half + 1).min(area.bottom());

                // Intensity increases as it collapses — CRT phosphor concentrating
                let brightness = 120 + (135.0 * t) as u8; // 120 → 255

                for y in top..bot {
                    // Distance from centre row normalised 0..1
                    let dist = if y >= cy { y - cy } else { cy - y } as f64 / half.max(1) as f64;
                    let row_bright = (brightness as f64 * (1.0 - dist * 0.6)) as u8;
                    let fg = Color::Rgb(0, row_bright, row_bright);

                    for x in area.left()..area.right() {
                        let cell = f.buffer_mut().get_mut(x, y);
                        cell.set_char('▒').set_fg(fg).set_bg(BG);
                    }
                }
            } else if elapsed < PHASE2_SQUEEZE_MS {
                // ── Phase 2: Horizontal squeeze to a dot ────────────
                let t = (elapsed - PHASE1_COLLAPSE_MS) as f64
                    / (PHASE2_SQUEEZE_MS - PHASE1_COLLAPSE_MS) as f64;
                // Ease-in: starts fast, slows near the end
                let eased = 1.0 - (1.0 - t).powi(2);
                let half_w = ((1.0 - eased) * w / 2.0).max(0.0) as u16;

                let left = cx.saturating_sub(half_w).max(area.left());
                let right = (cx + half_w + 1).min(area.right());

                // Draw the bright line on the centre row
                let beam_rows: &[u16] = if half_w > 2 {
                    &[cy.saturating_sub(1), cy, cy + 1]
                } else {
                    &[cy]
                };

                for &y in beam_rows {
                    if y >= area.top() && y < area.bottom() {
                        let is_centre = y == cy;
                        for x in left..right {
                            let dist_x = if x >= cx { x - cx } else { cx - x } as f64
                                / half_w.max(1) as f64;
                            let base: u8 = if is_centre { 255 } else { 140 };
                            let bright = (base as f64 * (1.0 - dist_x * 0.4)) as u8;
                            let fg = Color::Rgb(bright / 3, bright, bright);
                            let cell = f.buffer_mut().get_mut(x, y);
                            cell.set_char('━').set_fg(fg).set_bg(BG);
                        }
                    }
                }
            } else if elapsed < PHASE3_DOT_HOLD_MS {
                // ── Phase 3: Bright dot with phosphor glow ──────────
                let t = (elapsed - PHASE2_SQUEEZE_MS) as f64
                    / (PHASE3_DOT_HOLD_MS - PHASE2_SQUEEZE_MS) as f64;
                let brightness = 255 - (80.0 * t) as u8; // slow decay 255→175

                // Glow radius shrinks over time
                let glow_r: u16 = (3.0 * (1.0 - t)).max(1.0) as u16;

                for dy in 0..=glow_r {
                    for dx in 0..=glow_r * 2 {
                        // Characters are ~2x tall, so horizontal glow is wider
                        let dist = ((dx as f64 / 2.0).powi(2) + (dy as f64).powi(2)).sqrt();
                        if dist > glow_r as f64 {
                            continue;
                        }
                        let falloff = 1.0 - (dist / glow_r as f64);
                        let b = (brightness as f64 * falloff) as u8;
                        let fg = Color::Rgb(b / 3, b, b);

                        // Draw in all four quadrants
                        for &(sx, sy) in &[(1i16, 1i16), (1, -1), (-1, 1), (-1, -1)] {
                            let px = (cx as i16 + dx as i16 * sx) as u16;
                            let py = (cy as i16 + dy as i16 * sy) as u16;
                            if px >= area.left() && px < area.right()
                                && py >= area.top() && py < area.bottom()
                            {
                                let ch = if dx == 0 && dy == 0 { '●' } else { '·' };
                                let cell = f.buffer_mut().get_mut(px, py);
                                cell.set_char(ch).set_fg(fg).set_bg(BG);
                            }
                        }
                    }
                }
            } else {
                // ── Phase 4: Fade to black ──────────────────────────
                let t = (elapsed - PHASE3_DOT_HOLD_MS) as f64
                    / (PHASE4_FADE_MS - PHASE3_DOT_HOLD_MS) as f64;
                let brightness = (175.0 * (1.0 - t).max(0.0)) as u8;

                if brightness > 5 {
                    let fg = Color::Rgb(brightness / 4, brightness, brightness);
                    let cell = f.buffer_mut().get_mut(cx, cy);
                    cell.set_char('·').set_fg(fg).set_bg(BG);
                }
            }
        })?;

        // Any keypress skips the animation
        if crossterm::event::poll(Duration::from_millis(16))? {
            if let Event::Key(_) = event::read()? {
                break;
            }
        }
    }

    // One final black frame so the terminal is clean before we leave
    terminal.draw(|f| {
        let area = f.size();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = f.buffer_mut().get_mut(x, y);
                cell.set_char(' ').set_fg(BG).set_bg(BG);
            }
        }
    })?;

    Ok(())
}