use ratatui::style::Color;

/// A complete color theme for the player UI.
/// Does not affect the launcher, main menu, exit animation, or profiler overlay.
#[derive(Clone)]
pub struct Theme {
    pub name: &'static str,

    // ── Primary accents ──
    /// Primary accent: borders, active indicators, titles (default: cyan)
    pub accent: Color,
    /// Secondary accent: overlays, genre picker border (default: magenta)
    pub secondary: Color,
    /// Positive/active: selected items, playing status, good values (default: neon green)
    pub positive: Color,

    // ── Text ──
    /// Standard muted text: labels, secondary info (default: dim white)
    pub text_muted: Color,
    /// Warning/highlight text: bitrate, genre tags (default: yellow)
    pub text_warn: Color,
    /// Alert/error text: errors, high values (default: red)
    pub text_error: Color,
    /// Warm accent: country, secondary highlights (default: orange)
    pub text_warm: Color,

    // ── Backgrounds ──
    /// Main dark background (default: deep navy)
    pub bg_dark: Color,
    /// Panel background (default: slightly lighter navy)
    pub bg_panel: Color,
    /// Highlighted/selected row background (default: muted indigo)
    pub bg_highlight: Color,

    // ── Visualizer ──
    /// Peak indicator color on spectrum bars (default: white)
    pub peak: Color,
}

impl Theme {
    /// When `transparent` is true, clears the whole-window backgrounds
    /// (`bg_dark`, `bg_panel`) to `Color::Reset` so the terminal's own
    /// background — and its transparency, if the terminal emulator provides
    /// it (e.g. kitty, alacritty) — shows through instead. `bg_highlight` is
    /// deliberately left untouched: selected/highlighted rows need to stay
    /// visually distinct from the transparent background to remain readable.
    pub fn with_transparency(mut self, transparent: bool) -> Self {
        if transparent {
            self.bg_dark = Color::Reset;
            self.bg_panel = Color::Reset;
        }
        self
    }

    pub fn all() -> Vec<Theme> {
        vec![
            Self::crt(),
            Self::gruvbox(),
            Self::nord(),
            Self::dracula(),
            Self::monokai(),
            Self::catppuccin(),
            Self::hacker(),
            Self::solarized(),
        ]
    }

    pub fn by_name(name: &str) -> Theme {
        Self::all()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(Self::crt)
    }

    /// Default CRT phosphor theme — the original AetherTune aesthetic
    pub fn crt() -> Theme {
        Theme {
            name: "CRT",
            accent: Color::Rgb(0, 255, 255),       // cyan
            secondary: Color::Rgb(200, 80, 255),    // magenta
            positive: Color::Rgb(57, 255, 20),      // neon green
            text_muted: Color::Rgb(160, 160, 180),  // dim white
            text_warn: Color::Rgb(255, 215, 0),     // yellow
            text_error: Color::Rgb(255, 60, 60),    // red
            text_warm: Color::Rgb(255, 140, 0),     // orange
            bg_dark: Color::Rgb(15, 15, 25),
            bg_panel: Color::Rgb(20, 20, 35),
            bg_highlight: Color::Rgb(40, 40, 80),
            peak: Color::Rgb(255, 255, 255),
        }
    }

    /// Gruvbox — warm retro palette
    pub fn gruvbox() -> Theme {
        Theme {
            name: "Gruvbox",
            accent: Color::Rgb(131, 165, 152),      // aqua
            secondary: Color::Rgb(211, 134, 155),    // purple
            positive: Color::Rgb(184, 187, 38),      // green
            text_muted: Color::Rgb(168, 153, 132),   // gray
            text_warn: Color::Rgb(250, 189, 47),     // yellow
            text_error: Color::Rgb(251, 73, 52),     // red
            text_warm: Color::Rgb(254, 128, 25),     // orange
            bg_dark: Color::Rgb(40, 40, 40),
            bg_panel: Color::Rgb(50, 48, 47),
            bg_highlight: Color::Rgb(80, 73, 69),
            peak: Color::Rgb(235, 219, 178),         // fg
        }
    }

    /// Nord — cool arctic palette
    pub fn nord() -> Theme {
        Theme {
            name: "Nord",
            accent: Color::Rgb(136, 192, 208),       // frost blue
            secondary: Color::Rgb(180, 142, 173),     // purple
            positive: Color::Rgb(163, 190, 140),       // green
            text_muted: Color::Rgb(127, 140, 141),     // muted
            text_warn: Color::Rgb(235, 203, 139),       // yellow
            text_error: Color::Rgb(191, 97, 106),       // red
            text_warm: Color::Rgb(208, 135, 112),       // orange
            bg_dark: Color::Rgb(46, 52, 64),
            bg_panel: Color::Rgb(59, 66, 82),
            bg_highlight: Color::Rgb(76, 86, 106),
            peak: Color::Rgb(229, 233, 240),
        }
    }

    /// Dracula — dark purple theme
    pub fn dracula() -> Theme {
        Theme {
            name: "Dracula",
            accent: Color::Rgb(139, 233, 253),       // cyan
            secondary: Color::Rgb(255, 121, 198),     // pink
            positive: Color::Rgb(80, 250, 123),        // green
            text_muted: Color::Rgb(98, 114, 164),      // comment
            text_warn: Color::Rgb(241, 250, 140),       // yellow
            text_error: Color::Rgb(255, 85, 85),        // red
            text_warm: Color::Rgb(255, 184, 108),        // orange
            bg_dark: Color::Rgb(40, 42, 54),
            bg_panel: Color::Rgb(48, 50, 65),
            bg_highlight: Color::Rgb(68, 71, 90),
            peak: Color::Rgb(248, 248, 242),
        }
    }

    /// Monokai — classic editor theme
    pub fn monokai() -> Theme {
        Theme {
            name: "Monokai",
            accent: Color::Rgb(102, 217, 239),        // blue
            secondary: Color::Rgb(174, 129, 255),      // purple
            positive: Color::Rgb(166, 226, 46),         // green
            text_muted: Color::Rgb(117, 113, 94),       // comment
            text_warn: Color::Rgb(230, 219, 116),        // yellow
            text_error: Color::Rgb(249, 38, 114),        // red/pink
            text_warm: Color::Rgb(253, 151, 31),          // orange
            bg_dark: Color::Rgb(39, 40, 34),
            bg_panel: Color::Rgb(49, 50, 44),
            bg_highlight: Color::Rgb(73, 72, 62),
            peak: Color::Rgb(248, 248, 242),
        }
    }

    /// Catppuccin Mocha — pastel dark theme
    pub fn catppuccin() -> Theme {
        Theme {
            name: "Catppuccin",
            accent: Color::Rgb(137, 180, 250),         // blue
            secondary: Color::Rgb(203, 166, 247),       // mauve
            positive: Color::Rgb(166, 227, 161),         // green
            text_muted: Color::Rgb(147, 153, 178),       // subtext
            text_warn: Color::Rgb(249, 226, 175),         // yellow
            text_error: Color::Rgb(243, 139, 168),         // red
            text_warm: Color::Rgb(250, 179, 135),           // peach
            bg_dark: Color::Rgb(30, 30, 46),
            bg_panel: Color::Rgb(36, 36, 54),
            bg_highlight: Color::Rgb(49, 50, 68),
            peak: Color::Rgb(205, 214, 244),
        }
    }

    /// Hacker — green on black terminal aesthetic
    pub fn hacker() -> Theme {
        Theme {
            name: "Hacker",
            accent: Color::Rgb(0, 255, 65),             // matrix green
            secondary: Color::Rgb(0, 200, 50),           // darker green
            positive: Color::Rgb(50, 255, 100),           // bright green
            text_muted: Color::Rgb(0, 140, 40),           // dim green
            text_warn: Color::Rgb(200, 255, 0),            // yellow-green
            text_error: Color::Rgb(255, 50, 50),            // red (only non-green)
            text_warm: Color::Rgb(100, 255, 50),            // lime
            bg_dark: Color::Rgb(0, 5, 0),
            bg_panel: Color::Rgb(0, 10, 0),
            bg_highlight: Color::Rgb(0, 30, 5),
            peak: Color::Rgb(150, 255, 150),
        }
    }

    /// Solarized Dark — precision colors for readability
    pub fn solarized() -> Theme {
        Theme {
            name: "Solarized",
            accent: Color::Rgb(38, 139, 210),           // blue
            secondary: Color::Rgb(108, 113, 196),        // violet
            positive: Color::Rgb(133, 153, 0),            // green
            text_muted: Color::Rgb(88, 110, 117),         // base01
            text_warn: Color::Rgb(181, 137, 0),            // yellow
            text_error: Color::Rgb(220, 50, 47),            // red
            text_warm: Color::Rgb(203, 75, 22),              // orange
            bg_dark: Color::Rgb(0, 43, 54),
            bg_panel: Color::Rgb(7, 54, 66),
            bg_highlight: Color::Rgb(28, 78, 93),
            peak: Color::Rgb(238, 232, 213),
        }
    }
}