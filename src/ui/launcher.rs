use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Widget},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ── CRT effect character sets ───────────────────────────────────────
const GLITCH_CHARS: &[char] = &[
    '█', '▓', '▒', '░', '▄', '▀', '■', '□', '╬', '╠', '╣', '═', '║',
    '·', ':', '!', '@', '#', '$', '%', '^', '&', '*',
];
const NOISE_CHARS: &[char] = &[
    '░', '▒', '▓', '│', '─', '┼', '╬', '·', ':', ';', '!', '?', '$', '#', '@', '%',
];

// ── CRT boot timing (base milliseconds at Normal speed) ────────────
const CRT_FLASH_MS: u64 = 60;
const CRT_GLITCH_END_MS: u64 = 200;
const CRT_PHOSPHOR_END_MS: u64 = 500;
const CRT_STATIC_END_MS: u64 = 650;
const CRT_BEAM_END_MS: u64 = 1100;
const CRT_TOTAL_MS: u64 = 1200;

// ── Boot animation base timing ─────────────────────────────────────
const BASE_LOGO_DURATION_MS: u64 = 1400;
const BASE_GEAR_GAP_MS: u64 = 300;
const BASE_GEAR_DURATION_MS: u64 = 2400;
const BASE_MENU_GAP_MS: u64 = 300;
const BASE_MENU_SLIDE_MS: u64 = 500;
const BASE_CONNECT_DONE_MS: u64 = 3200;

/// Boot animation speed presets
#[derive(Clone, Copy)]
pub enum BootSpeed {
    Fast,   // 0.5x duration
    Normal, // 1.0x — default, a bit slower than before
    Slow,   // 1.8x — dramatic
    Off,    // skip CRT + boot, go straight to menu
}

impl BootSpeed {
    fn multiplier(self) -> f64 {
        match self {
            BootSpeed::Fast => 0.5,
            BootSpeed::Normal => 1.0,
            BootSpeed::Slow => 1.8,
            BootSpeed::Off => 0.0,
        }
    }
}

/// Pre-computed timing thresholds derived from speed setting
struct Timing {
    crt_total: u64,
    logo_duration: u64,
    gear_start: u64,
    gear_duration: u64,
    menu_start: u64,
    menu_slide: u64,
    anim_done: u64,
    connect_done: u64,
}

impl Timing {
    fn new(speed: BootSpeed) -> Self {
        let m = speed.multiplier();
        let logo = (BASE_LOGO_DURATION_MS as f64 * m) as u64;
        let gear_gap = (BASE_GEAR_GAP_MS as f64 * m) as u64;
        let gear = (BASE_GEAR_DURATION_MS as f64 * m) as u64;
        let menu_gap = (BASE_MENU_GAP_MS as f64 * m) as u64;
        let menu_slide = (BASE_MENU_SLIDE_MS as f64 * m) as u64;
        let gear_start = logo + gear_gap;
        let menu_start = gear_start + gear + menu_gap;
        let anim_done = menu_start + menu_slide;
        let connect = (BASE_CONNECT_DONE_MS as f64 * m) as u64;
        let crt = (CRT_TOTAL_MS as f64 * m) as u64;

        Self {
            crt_total: crt,
            logo_duration: logo,
            gear_start,
            gear_duration: gear,
            menu_start,
            menu_slide,
            anim_done,
            connect_done: connect,
        }
    }
}

const LOGO_LINES: [&str; 6] = [
    r"  █████╗ ███████╗████████╗██╗  ██╗███████╗██████╗ ████████╗██╗   ██╗███╗   ██╗███████╗",
    r" ██╔══██╗██╔════╝╚══██╔══╝██║  ██║██╔════╝██╔══██╗╚══██╔══╝██║   ██║████╗  ██║██╔════╝",
    r" ███████║█████╗     ██║   ███████║█████╗  ██████╔╝   ██║   ██║   ██║██╔██╗ ██║█████╗  ",
    r" ██╔══██║██╔══╝     ██║   ██╔══██║██╔══╝  ██╔══██╗   ██║   ██║   ██║██║╚██╗██║██╔══╝  ",
    r" ██║  ██║███████╗   ██║   ██║  ██║███████╗██║  ██║   ██║   ╚██████╔╝██║ ╚████║███████╗",
    r" ╚═╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝   ╚═╝    ╚═════╝ ╚═╝  ╚═══╝╚══════╝",
];

/// Gear spinner frames — each frame is one rotation step
const GEAR_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Boot sequence status messages shown alongside the gear
const BOOT_MESSAGES: [&str; 5] = [
    "Initializing audio subsystem…",
    "Loading frequency analyzer…",
    "Calibrating spectrum bands…",
    "Connecting to RadioBrowser API…",
    "Ready.",
];

/// Connect sequence status messages (timestamps are proportional, scaled by speed)
fn connect_messages(total_ms: u64) -> [(& 'static str, u64); 6] {
    let step = total_ms / 7;
    [
        ("Tuning into RadioBrowser API…",   0),
        ("Querying station directory…",     step),
        ("Loading station metadata…",       step * 2),
        ("Initializing audio pipeline…",    step * 3),
        ("Configuring spectrum analyzer…",  step * 4),
        ("Launching AetherTune ✓",          step * 5),
    ]
}

#[derive(PartialEq)]
enum MenuState {
    CrtBoot,
    Boot,
    Main,
    About,
    Settings,
    Connecting,
}

struct MenuApp {
    selected: usize,
    state: MenuState,
    options: Vec<(&'static str, &'static str)>,
    boot_start: Instant,
    connect_start: Option<Instant>,
    timing: Timing,
    /// Country code input buffer for settings screen
    settings_country: String,
}

impl MenuApp {
    fn new(speed: BootSpeed) -> Self {
        let initial_state = match speed {
            BootSpeed::Off => MenuState::Main,
            _ => MenuState::CrtBoot,
        };

        // Load existing country code from config
        let config = crate::storage::config::Config::load();

        Self {
            selected: 0,
            state: initial_state,
            options: vec![
                ("Start Radio", "Browse and stream internet radio stations"),
                ("Settings", "Configure country and preferences"),
                ("About", "Version info and credits"),
                ("Quit", "Exit AetherTune"),
            ],
            boot_start: Instant::now(),
            connect_start: None,
            timing: Timing::new(speed),
            settings_country: config.country_code,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.boot_start.elapsed().as_millis() as u64
    }
}

/// Show the launch menu. Returns true to start radio, false to quit.
pub fn show(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, speed: BootSpeed) -> io::Result<bool> {
    let mut menu = MenuApp::new(speed);

    loop {
        terminal.draw(|f| match menu.state {
            MenuState::CrtBoot => draw_crt(f, &menu),
            MenuState::Boot => draw_boot(f, &menu),
            MenuState::Main => draw_main(f, &menu),
            MenuState::About => draw_about(f),
            MenuState::Settings => draw_settings(f, &menu),
            MenuState::Connecting => draw_connecting(f, &menu),
        })?;

        // During animations, use a fast poll for smooth rendering
        let poll_ms = match menu.state {
            MenuState::CrtBoot | MenuState::Boot | MenuState::Connecting => 16, // ~60fps for CRT
            _ => 100,
        };

        if crossterm::event::poll(Duration::from_millis(poll_ms))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match menu.state {
                    MenuState::CrtBoot | MenuState::Boot => {
                        // Any key skips to main menu
                        menu.state = MenuState::Main;
                    }
                    MenuState::Main => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if menu.selected > 0 {
                                menu.selected -= 1;
                            } else {
                                menu.selected = menu.options.len() - 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            menu.selected = (menu.selected + 1) % menu.options.len();
                        }
                        KeyCode::Enter => match menu.selected {
                            0 => {
                                menu.connect_start = Some(Instant::now());
                                menu.state = MenuState::Connecting;
                            }
                            1 => menu.state = MenuState::Settings,
                            2 => menu.state = MenuState::About,
                            3 => return Ok(false),
                            _ => {}
                        },
                        KeyCode::Char('q') => return Ok(false),
                        _ => {}
                    },
                    MenuState::About => match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            menu.state = MenuState::Main;
                        }
                        _ => {}
                    },
                    MenuState::Settings => match key.code {
                        KeyCode::Esc => {
                            // Save and return to main menu
                            let mut config = crate::storage::config::Config::load();
                            config.country_code = menu.settings_country.clone().to_uppercase();
                            config.save();
                            menu.settings_country = config.country_code.clone();
                            menu.state = MenuState::Main;
                        }
                        KeyCode::Enter => {
                            // Save and return to main menu
                            let mut config = crate::storage::config::Config::load();
                            config.country_code = menu.settings_country.clone().to_uppercase();
                            config.save();
                            menu.settings_country = config.country_code.clone();
                            menu.state = MenuState::Main;
                        }
                        KeyCode::Char(c) if menu.settings_country.len() < 2 => {
                            if c.is_ascii_alphabetic() {
                                menu.settings_country.push(c.to_ascii_uppercase());
                            }
                        }
                        KeyCode::Backspace => {
                            menu.settings_country.pop();
                        }
                        _ => {}
                    },
                    MenuState::Connecting => {
                        // No key handling during connect animation
                    }
                }
            }
        }

        // Auto-transition from CRT to logo boot
        if menu.state == MenuState::CrtBoot && menu.elapsed_ms() > menu.timing.crt_total {
            menu.boot_start = Instant::now(); // Reset timer for logo boot phase
            menu.state = MenuState::Boot;
        }

        // Auto-transition from boot to main when animation finishes
        if menu.state == MenuState::Boot && menu.elapsed_ms() > menu.timing.anim_done {
            menu.state = MenuState::Main;
        }

        // Auto-transition from connecting to app when animation finishes
        if menu.state == MenuState::Connecting {
            if let Some(start) = menu.connect_start {
                if start.elapsed().as_millis() as u64 > menu.timing.connect_done {
                    return Ok(true);
                }
            }
        }
    }
}

// ── CRT power-on animation ──────────────────────────────────────────

/// A widget that fills an area with random noise characters
struct NoiseWidget {
    seed: u64,
    palette: Vec<Color>,
}

impl Widget for NoiseWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut rng_state = self.seed;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                // Simple LCG for deterministic-per-frame randomness
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let char_idx = (rng_state >> 16) as usize % NOISE_CHARS.len();
                let color_idx = (rng_state >> 24) as usize % self.palette.len();
                { let cell = buf.get_mut(x, y);
                    cell.set_char(NOISE_CHARS[char_idx])
                        .set_fg(self.palette[color_idx])
                        .set_bg(Color::Rgb(5, 5, 10));
                }
            }
        }
    }
}

fn draw_crt(f: &mut Frame, menu: &MenuApp) {
    let area = f.size();
    let elapsed = menu.elapsed_ms();

    if elapsed < CRT_FLASH_MS {
        // ── Phase 1: White flash ────────────────────────────────────
        let brightness = if elapsed < CRT_FLASH_MS / 2 { 255 } else { 180 };
        let flash_color = Color::Rgb(brightness, brightness, brightness);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                { let cell = f.buffer_mut().get_mut(x, y);
                    cell.set_char('█')
                        .set_fg(flash_color)
                        .set_bg(flash_color);
                }
            }
        }
    } else if elapsed < CRT_GLITCH_END_MS {
        // ── Phase 2: Glitch burst ───────────────────────────────────
        // Dark screen with scattered glitch characters
        let bg = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
        f.render_widget(bg, area);

        let mut rng = rand::rng();
        let glitch_count = 8 + ((elapsed - CRT_FLASH_MS) / 20) as usize;
        let fringe_colors = [
            Color::Rgb(0, 200, 200),
            Color::Rgb(0, 255, 255),
            Color::Rgb(80, 255, 200),
            Color::Rgb(0, 180, 180),
            Color::Rgb(100, 100, 200),
        ];

        for _ in 0..glitch_count {
            let row = rng.random_range(area.top()..area.bottom());
            let col_start = rng.random_range(area.left()..area.right().saturating_sub(20));
            let length = rng.random_range(8..25).min((area.right() - col_start) as usize);

            for j in 0..length {
                let x = col_start + j as u16;
                if x < area.right() {
                    let ch = GLITCH_CHARS[rng.random_range(0..GLITCH_CHARS.len())];
                    let color = fringe_colors[rng.random_range(0..fringe_colors.len())];
                    { let cell = f.buffer_mut().get_mut(x, row);
                        cell.set_char(ch)
                            .set_fg(color)
                            .set_bg(Color::Rgb(5, 5, 10));
                    }
                }
            }
        }
    } else if elapsed < CRT_PHOSPHOR_END_MS {
        // ── Phase 3: Phosphor ramp ──────────────────────────────────
        // Screen fills with progressively brighter block characters
        let phase_progress =
            (elapsed - CRT_GLITCH_END_MS) as f64 / (CRT_PHOSPHOR_END_MS - CRT_GLITCH_END_MS) as f64;

        let (ch, fg) = if phase_progress < 0.2 {
            ('▓', Color::Rgb(0, 40, 0))
        } else if phase_progress < 0.4 {
            ('▓', Color::Rgb(0, 80, 40))
        } else if phase_progress < 0.6 {
            ('▒', Color::Rgb(0, 120, 80))
        } else if phase_progress < 0.8 {
            ('▒', Color::Rgb(0, 180, 160))
        } else {
            ('░', Color::Rgb(0, 220, 220))
        };

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                { let cell = f.buffer_mut().get_mut(x, y);
                    cell.set_char(ch)
                        .set_fg(fg)
                        .set_bg(Color::Rgb(5, 5, 10));
                }
            }
        }
    } else if elapsed < CRT_STATIC_END_MS {
        // ── Phase 4: Static noise burst ─────────────────────────────
        let noise = NoiseWidget {
            seed: elapsed * 7919, // Different pattern each frame
            palette: vec![
                Color::Rgb(0, 180, 180),
                Color::Rgb(0, 255, 255),
                Color::Rgb(0, 200, 200),
                Color::Rgb(0, 140, 140),
            ],
        };
        f.render_widget(noise, area);
    } else if elapsed < CRT_BEAM_END_MS {
        // ── Phase 5: Beam sweep ─────────────────────────────────────
        // A bright horizontal line sweeps top-to-bottom
        let bg = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
        f.render_widget(bg, area);

        let sweep_progress =
            (elapsed - CRT_STATIC_END_MS) as f64 / (CRT_BEAM_END_MS - CRT_STATIC_END_MS) as f64;
        let beam_row = area.top() + (sweep_progress * area.height as f64) as u16;

        for y in area.top()..area.bottom() {
            let dist = if y > beam_row {
                y - beam_row
            } else {
                beam_row - y
            };

            let (ch, color) = if dist == 0 {
                ('━', Color::Rgb(0, 255, 255)) // Bright beam
            } else if dist == 1 {
                ('─', Color::Rgb(0, 180, 180)) // Near trail
            } else if dist <= 3 && y < beam_row {
                ('─', Color::Rgb(0, 80, 80)) // Fading trail behind
            } else {
                continue;
            };

            for x in area.left()..area.right() {
                { let cell = f.buffer_mut().get_mut(x, y);
                    cell.set_char(ch)
                        .set_fg(color)
                        .set_bg(Color::Rgb(5, 5, 10));
                }
            }
        }
    } else {
        // ── Phase 6: Clear → transition to Boot ─────────────────────
        let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
        f.render_widget(bg, area);
    }
}

// ── Boot animation ──────────────────────────────────────────────────

fn draw_boot(f: &mut Frame, menu: &MenuApp) {
    let area = f.size();
    let elapsed = menu.elapsed_ms();

    let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
    f.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Top padding
            Constraint::Length(8),  // Logo
            Constraint::Length(1),  // Spacer
            Constraint::Length(3),  // Gear + boot message
            Constraint::Length(2),  // Spacer
            Constraint::Min(8),    // Menu area (slides in)
            Constraint::Length(3), // Hints
        ])
        .split(area);

    // ── Phase 1: Logo typewriter ────────────────────────────────────
    let logo_progress = if elapsed >= menu.timing.logo_duration {
        1.0
    } else {
        elapsed as f64 / menu.timing.logo_duration as f64
    };

    // Total characters across all logo lines
    let total_chars: usize = LOGO_LINES.iter().map(|l| l.chars().count()).sum();
    let chars_to_show = (total_chars as f64 * logo_progress) as usize;

    let mut shown = 0usize;
    let mut logo_lines: Vec<Line> = Vec::new();

    for logo_line in &LOGO_LINES {
        let line_len = logo_line.chars().count();
        if shown >= chars_to_show {
            // This line hasn't started yet
            logo_lines.push(Line::from(""));
        } else {
            let visible = (chars_to_show - shown).min(line_len);
            let visible_str: String = logo_line.chars().take(visible).collect();

            // Color: revealed chars in cyan, with a bright "cursor" at the edge
            let mut spans = vec![Span::styled(
                visible_str.clone(),
                Style::default().fg(Color::Rgb(0, 255, 255)),
            )];

            // Blinking cursor at the typing edge (only during typing phase)
            if visible < line_len && elapsed < menu.timing.logo_duration {
                let cursor_char = if (elapsed / 80) % 2 == 0 { "█" } else { "▌" };
                spans.push(Span::styled(
                    cursor_char,
                    Style::default().fg(Color::Rgb(0, 200, 200)),
                ));
            }

            logo_lines.push(Line::from(spans));
        }
        shown += line_len;
    }

    let logo = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo, chunks[1]);

    // ── Phase 2: Gear spinner + boot messages ───────────────────────
    if elapsed >= menu.timing.gear_start {
        let gear_elapsed = elapsed - menu.timing.gear_start;
        let gear_frame = ((gear_elapsed / 80) % GEAR_FRAMES.len() as u64) as usize;
        let gear_char = GEAR_FRAMES[gear_frame];

        // Which boot message to show (cycle through them)
        let msg_idx = if gear_elapsed >= menu.timing.gear_duration {
            BOOT_MESSAGES.len() - 1 // "Ready."
        } else {
            let step = menu.timing.gear_duration / (BOOT_MESSAGES.len() as u64 - 1);
            ((gear_elapsed / step) as usize).min(BOOT_MESSAGES.len() - 2)
        };
        let msg = BOOT_MESSAGES[msg_idx];

        let is_ready = msg_idx == BOOT_MESSAGES.len() - 1;
        let gear_color = if is_ready {
            Color::Rgb(57, 255, 20) // Green when ready
        } else {
            Color::Rgb(0, 255, 255) // Cyan during loading
        };
        let msg_color = if is_ready {
            Color::Rgb(57, 255, 20)
        } else {
            Color::Rgb(100, 100, 140)
        };

        let gear_line = Line::from(vec![
            Span::styled(
                format!("  {} ", gear_char),
                Style::default().fg(gear_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg, Style::default().fg(msg_color)),
        ]);

        let gear_widget = Paragraph::new(gear_line).alignment(Alignment::Center);
        f.render_widget(gear_widget, chunks[3]);
    }

    // ── Phase 3: Menu slide-in ──────────────────────────────────────
    if elapsed >= menu.timing.menu_start {
        let menu_elapsed = elapsed - menu.timing.menu_start;
        let menu_progress = if menu_elapsed >= menu.timing.menu_slide {
            1.0
        } else {
            menu_elapsed as f64 / menu.timing.menu_slide as f64
        };

        // How many menu items to show (slide in one by one)
        let items_visible = ((menu.options.len() as f64 * menu_progress).ceil() as usize)
            .min(menu.options.len());

        draw_menu_box(f, menu, chunks[5], items_visible);
    }

    // ── Bottom hint: skip ────────────────────────────────────────────
    if elapsed < menu.timing.anim_done {
        let skip_hint = Paragraph::new(Line::from(Span::styled(
            "Press any key to skip",
            Style::default().fg(Color::Rgb(50, 50, 70)),
        )))
        .alignment(Alignment::Center);
        f.render_widget(skip_hint, chunks[6]);
    } else {
        draw_hints(f, chunks[6]);
    }
}

// ── Main menu (post-animation) ──────────────────────────────────────

fn draw_main(f: &mut Frame, menu: &MenuApp) {
    let area = f.size();

    let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
    f.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Top padding
            Constraint::Length(8),  // Logo
            Constraint::Length(2),  // Subtitle
            Constraint::Min(8),    // Menu
            Constraint::Length(3), // Hints
        ])
        .split(area);

    // Full logo
    let logo_lines: Vec<Line> = LOGO_LINES
        .iter()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(Color::Rgb(0, 255, 255)),
            ))
        })
        .collect();
    let logo = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo, chunks[1]);

    // Subtitle
    let subtitle = Paragraph::new(Line::from(vec![
        Span::styled(
            "Terminal Radio Player",
            Style::default().fg(Color::Rgb(80, 80, 120)),
        ),
        Span::styled("  │  ", Style::default().fg(Color::Rgb(40, 40, 60))),
        Span::styled(
            "Real-Time Audio Visualization",
            Style::default().fg(Color::Rgb(80, 80, 120)),
        ),
        Span::styled("  │  ", Style::default().fg(Color::Rgb(40, 40, 60))),
        Span::styled(
            format!("v{}", VERSION),
            Style::default().fg(Color::Rgb(80, 80, 120)),
        ),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(subtitle, chunks[2]);

    // Menu
    draw_menu_box(f, menu, chunks[3], menu.options.len());

    // Hints
    draw_hints(f, chunks[4]);
}

// ── Shared components ───────────────────────────────────────────────

fn draw_menu_box(f: &mut Frame, menu: &MenuApp, area: Rect, items_visible: usize) {
    let menu_width = 60u16.min(area.width.saturating_sub(4));
    let menu_x = area.x + area.width.saturating_sub(menu_width) / 2;
    let menu_height = (menu.options.len() as u16 * 3 + 2).min(area.height);
    let menu_area = Rect::new(menu_x, area.y, menu_width, menu_height);

    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 100)))
        .padding(Padding::new(2, 2, 1, 0))
        .style(Style::default().bg(Color::Rgb(18, 18, 30)));
    let inner = menu_block.inner(menu_area);
    f.render_widget(menu_block, menu_area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, (label, desc)) in menu.options.iter().enumerate() {
        if i >= items_visible {
            break;
        }

        let is_selected = i == menu.selected;

        if is_selected {
            lines.push(Line::from(vec![
                Span::styled(
                    " ❯ ",
                    Style::default()
                        .fg(Color::Rgb(0, 255, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    label.to_string(),
                    Style::default()
                        .fg(Color::Rgb(0, 255, 255))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled(
                    desc.to_string(),
                    Style::default().fg(Color::Rgb(100, 100, 140)),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    label.to_string(),
                    Style::default().fg(Color::Rgb(80, 80, 100)),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled(
                    desc.to_string(),
                    Style::default().fg(Color::Rgb(50, 50, 70)),
                ),
            ]));
        }

        if i < items_visible.min(menu.options.len()) - 1 {
            lines.push(Line::from(""));
        }
    }

    let menu_widget = Paragraph::new(lines);
    f.render_widget(menu_widget, inner);
}

fn draw_hints(f: &mut Frame, area: Rect) {
    let hints = Paragraph::new(Line::from(vec![
        Span::styled("↑ ↓ ", Style::default().fg(Color::Rgb(0, 255, 255))),
        Span::styled("navigate", Style::default().fg(Color::Rgb(80, 80, 100))),
        Span::styled("    Enter ", Style::default().fg(Color::Rgb(0, 255, 255))),
        Span::styled("select", Style::default().fg(Color::Rgb(80, 80, 100))),
        Span::styled("    q ", Style::default().fg(Color::Rgb(0, 255, 255))),
        Span::styled("quit", Style::default().fg(Color::Rgb(80, 80, 100))),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hints, area);
}

// ── Connect transition ──────────────────────────────────────────────

fn draw_connecting(f: &mut Frame, menu: &MenuApp) {
    let area = f.size();
    let elapsed = menu
        .connect_start
        .map(|s| s.elapsed().as_millis() as u64)
        .unwrap_or(0);

    let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
    f.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(16),
            Constraint::Min(0),
        ])
        .split(area);

    // Centered box
    let box_width = 62u16.min(area.width.saturating_sub(4));
    let box_height = 14u16;
    let box_x = chunks[1].x + chunks[1].width.saturating_sub(box_width) / 2;
    let box_area = Rect::new(box_x, chunks[1].y, box_width, box_height.min(chunks[1].height));

    // Animated border: draw progressively
    let border_progress = (elapsed as f64 / 300.0).min(1.0);
    let border_color = if border_progress >= 1.0 {
        Color::Rgb(0, 255, 255)
    } else {
        let v = (border_progress * 255.0) as u8;
        Color::Rgb(0, v, v)
    };

    let block = Block::default()
        .title(Span::styled(
            " Connecting ",
            Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::new(2, 2, 1, 0))
        .style(Style::default().bg(Color::Rgb(18, 18, 30)));

    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Static info section
    lines.push(Line::from(vec![
        Span::styled("  Source     ", Style::default().fg(Color::Rgb(100, 100, 130))),
        Span::styled(
            "RadioBrowser API",
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Genre      ", Style::default().fg(Color::Rgb(100, 100, 130))),
        Span::styled("Lo-fi", Style::default().fg(Color::Rgb(255, 215, 0))),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Audio      ", Style::default().fg(Color::Rgb(100, 100, 130))),
        Span::styled("mpv → PipeWire", Style::default().fg(Color::Rgb(160, 160, 180))),
    ]));
    lines.push(Line::from(""));

    // Separator
    let sep_width = inner.width.saturating_sub(2) as usize;
    lines.push(Line::from(Span::styled(
        format!("  {}", "─".repeat(sep_width.min(50))),
        Style::default().fg(Color::Rgb(40, 40, 60)),
    )));
    lines.push(Line::from(""));

    // Animated status messages — each appears at its timestamp
    let msgs = connect_messages(menu.timing.connect_done);
    for &(msg, start_at) in &msgs {
        if elapsed < start_at {
            break;
        }

        let is_last = msg.contains('✓');
        let is_current = elapsed < start_at + 500 && !is_last;

        // Spinner for the current active step
        let prefix = if is_last {
            Span::styled(
                "  ✓ ",
                Style::default()
                    .fg(Color::Rgb(57, 255, 20))
                    .add_modifier(Modifier::BOLD),
            )
        } else if is_current {
            let frame = ((elapsed / 80) % GEAR_FRAMES.len() as u64) as usize;
            Span::styled(
                format!("  {} ", GEAR_FRAMES[frame]),
                Style::default()
                    .fg(Color::Rgb(0, 255, 255))
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "  ✓ ",
                Style::default().fg(Color::Rgb(60, 100, 60)),
            )
        };

        let msg_color = if is_last {
            Color::Rgb(57, 255, 20)
        } else if is_current {
            Color::Rgb(200, 200, 220)
        } else {
            Color::Rgb(60, 60, 80)
        };

        lines.push(Line::from(vec![
            prefix,
            Span::styled(msg, Style::default().fg(msg_color)),
        ]));
    }

    let content = Paragraph::new(lines);
    f.render_widget(content, inner);

    // Progress bar at bottom of box
    if elapsed > 200 {
        let progress = ((elapsed - 200) as f64 / (menu.timing.connect_done - 200) as f64).min(1.0);
        let bar_width = (box_width.saturating_sub(4)) as usize;
        let filled = (bar_width as f64 * progress) as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar_y = box_area.y + box_area.height.saturating_sub(2);
        if bar_y < area.height {
            let bar_area = Rect::new(box_area.x + 2, bar_y, box_width.saturating_sub(4), 1);

            let bar_color = if progress >= 1.0 {
                Color::Rgb(57, 255, 20)
            } else {
                Color::Rgb(0, 255, 255)
            };

            let bar = Paragraph::new(Line::from(vec![
                Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
                Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::Rgb(30, 30, 50)),
                ),
            ]));
            f.render_widget(bar, bar_area);
        }
    }
}

// ── About screen ────────────────────────────────────────────────────

fn draw_settings(f: &mut Frame, menu: &MenuApp) {
    let area = f.size();
    let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
    f.render_widget(bg, area);

    let box_width = 56u16;
    let box_height = 18u16;
    let box_x = area.width.saturating_sub(box_width) / 2;
    let box_y = area.height.saturating_sub(box_height) / 2;
    let box_area = Rect::new(
        box_x,
        box_y,
        box_width.min(area.width),
        box_height.min(area.height),
    );

    f.render_widget(Clear, box_area);

    let block = Block::default()
        .title(Span::styled(
            " Settings ",
            Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(0, 255, 255)))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(18, 18, 30)));

    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    // Country code display with cursor
    let country_display = if menu.settings_country.is_empty() {
        "__ ".to_string()
    } else if menu.settings_country.len() == 1 {
        format!("{}_ ", menu.settings_country)
    } else {
        format!("{} ", menu.settings_country)
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Country Code",
                Style::default()
                    .fg(Color::Rgb(0, 255, 255))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  ",
                Style::default(),
            ),
            Span::styled(
                country_display,
                Style::default()
                    .fg(Color::Rgb(57, 255, 20))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if menu.settings_country.len() < 2 { "│" } else { "✓" },
                Style::default().fg(if menu.settings_country.len() < 2 {
                    Color::Rgb(0, 255, 255)
                } else {
                    Color::Rgb(57, 255, 20)
                }),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  ISO 3166-1 Alpha-2 code (e.g. US, DE, GB)",
                Style::default().fg(Color::Rgb(80, 80, 110)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Blends ~30% local stations into results",
                Style::default().fg(Color::Rgb(80, 80, 110)),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Leave empty for global-only results",
                Style::default().fg(Color::Rgb(60, 60, 80)),
            ),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter ", Style::default().fg(Color::Rgb(0, 255, 255))),
            Span::styled("save  ", Style::default().fg(Color::Rgb(80, 80, 100))),
            Span::styled("  Esc ", Style::default().fg(Color::Rgb(0, 255, 255))),
            Span::styled("save & back  ", Style::default().fg(Color::Rgb(80, 80, 100))),
            Span::styled("  Bksp ", Style::default().fg(Color::Rgb(0, 255, 255))),
            Span::styled("clear", Style::default().fg(Color::Rgb(80, 80, 100))),
        ]),
    ];

    let settings = Paragraph::new(lines);
    f.render_widget(settings, inner);
}

fn draw_about(f: &mut Frame) {
    let area = f.size();
    let bg = Block::default().style(Style::default().bg(Color::Rgb(12, 12, 20)));
    f.render_widget(bg, area);

    let box_width = 56u16;
    let box_height = 16u16;
    let box_x = area.width.saturating_sub(box_width) / 2;
    let box_y = area.height.saturating_sub(box_height) / 2;
    let box_area = Rect::new(
        box_x,
        box_y,
        box_width.min(area.width),
        box_height.min(area.height),
    );

    f.render_widget(Clear, box_area);

    let block = Block::default()
        .title(Span::styled(
            " About AetherTune ",
            Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(0, 255, 255)))
        .padding(Padding::new(2, 2, 1, 1))
        .style(Style::default().bg(Color::Rgb(18, 18, 30)));

    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Version    ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(VERSION, Style::default().fg(Color::Rgb(57, 255, 20))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "License    ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled("MIT", Style::default().fg(Color::Rgb(255, 215, 0))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Repo       ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                "github.com/nevermore23274/AetherTune",
                Style::default().fg(Color::Rgb(100, 150, 255)),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Audio      ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                "mpv + PipeWire/PulseAudio",
                Style::default().fg(Color::Rgb(160, 160, 180)),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Visualizer ",
                Style::default().fg(Color::Rgb(100, 100, 130)),
            ),
            Span::styled(
                "FFT analysis, CAVA-inspired smoothing",
                Style::default().fg(Color::Rgb(160, 160, 180)),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc to return",
            Style::default().fg(Color::Rgb(60, 60, 80)),
        )),
    ];

    let about = Paragraph::new(lines);
    f.render_widget(about, inner);
}