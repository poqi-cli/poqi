use poqi_config::AppConfig;
use ratatui::style::{Color, Modifier, Style};

pub const AVAILABLE_THEMES: &[&str] = &[
    "dark",
    "light",
    "cyberpunk",
    "midnight",
    "nord",
    "synthwave",
];

#[derive(Debug, Clone)]
pub struct Theme {
    pub border: Style,
    pub border_focused: Style,
    pub border_cursor: Style,
    pub text: Style,
    pub text_muted: Style,
    pub accent_primary: Style,
    pub accent_secondary: Style,
    pub accent_tertiary: Style,
    pub highlight: Style,
    pub panel_background: Style,
    pub panel_background_selected: Style,
    pub panel_background_focused: Style,
    pub panel_title: Style,
    pub panel_title_selected: Style,
    pub panel_title_focused: Style,
    pub grid_selection: Style,
    pub grid_focus_row: Style,
    pub status_info: Style,
    pub status_warning: Style,
    pub status_success: Style,
    pub status_error: Style,
    pub syntax_keyword: Style,
    pub syntax_identifier: Style,
    pub syntax_literal: Style,
    pub syntax_number: Style,
    pub syntax_comment: Style,
    pub syntax_operator: Style,
    pub editor_selection: Style,
}

impl Theme {
    pub const fn names() -> &'static [&'static str] {
        AVAILABLE_THEMES
    }

    #[must_use]
    pub fn from_config(config: &AppConfig) -> Self {
        match config.ui.theme.as_str() {
            "light" => Self::light(),
            "nord" => Self::nord(),
            "synthwave" => Self::synthwave(),
            "midnight" | "high_contrast" => Self::midnight(),
            "cyberpunk" => Self::cyberpunk(),
            // Default theme (includes "dark", "classic", "classic_dark", and any unrecognized)
            _ => Self::dark(),
        }
    }

    /// Premium modern dark theme (Professional Slate)
    fn dark() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(100, 110, 120)), // sharper slate
            border_focused: Style::default()
                .fg(Color::Rgb(0, 200, 200)) // sharp teal
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(0, 200, 200)) // sharp teal
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(250, 250, 250)), // crisp white
            text_muted: Style::default().fg(Color::Rgb(140, 150, 160)), // muted slate
            accent_primary: Style::default()
                .fg(Color::Rgb(0, 200, 200)) // sharp teal
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default()
                .fg(Color::Rgb(0, 180, 180)) // darker teal
                .add_modifier(Modifier::BOLD),
            accent_tertiary: Style::default()
                .fg(Color::Rgb(220, 170, 60)) // muted gold
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(40, 50, 60)) // slate highlight
                .fg(Color::Rgb(255, 255, 255)),
            panel_background: Style::default().bg(Color::Rgb(15, 17, 20)), // deep sharp dark
            panel_background_selected: Style::default().bg(Color::Rgb(25, 27, 30)),
            panel_background_focused: Style::default().bg(Color::Rgb(20, 22, 25)),
            panel_title: Style::default().fg(Color::Rgb(180, 190, 200)), // brighter title
            panel_title_selected: Style::default()
                .fg(Color::Rgb(0, 200, 200)) // sharp teal
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(0, 200, 200)) // sharp teal
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(35, 45, 55))
                .fg(Color::Rgb(255, 255, 255)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(45, 55, 65))
                .fg(Color::Rgb(255, 255, 255)),
            status_info: Style::default().fg(Color::Rgb(0, 200, 200)), // sharp teal
            status_warning: Style::default().fg(Color::Rgb(220, 170, 60)), // gold
            status_success: Style::default().fg(Color::Rgb(60, 180, 100)), // green
            status_error: Style::default()
                .fg(Color::Rgb(220, 60, 60)) // red
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default().fg(Color::Rgb(0, 200, 200)), // sharp teal
            syntax_identifier: Style::default().fg(Color::Rgb(250, 250, 250)), // white
            syntax_literal: Style::default().fg(Color::Rgb(80, 200, 120)), // brighter green
            syntax_number: Style::default().fg(Color::Rgb(240, 190, 80)), // brighter gold
            syntax_comment: Style::default().fg(Color::Rgb(110, 120, 130)), // comment
            syntax_operator: Style::default().fg(Color::Rgb(180, 190, 200)), // slate
            editor_selection: Style::default()
                .bg(Color::Rgb(40, 50, 60))
                .fg(Color::Rgb(255, 255, 255)),
        }
    }

    /// Clean professional light theme (Classic IDE)
    fn light() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(180, 180, 180)), // neutral gray border
            border_focused: Style::default()
                .fg(Color::Rgb(0, 120, 180)) // professional blue
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(0, 120, 180)) // professional blue
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(30, 30, 30)), // near-black text
            text_muted: Style::default().fg(Color::Rgb(110, 110, 110)), // medium gray
            accent_primary: Style::default()
                .fg(Color::Rgb(0, 100, 160)) // deep professional blue
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default()
                .fg(Color::Rgb(80, 80, 80)) // dark gray accent
                .add_modifier(Modifier::BOLD),
            accent_tertiary: Style::default()
                .fg(Color::Rgb(0, 100, 160)) // consistent blue
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(220, 235, 250)) // soft blue highlight
                .fg(Color::Rgb(30, 30, 30)),
            panel_background: Style::default().bg(Color::Rgb(250, 250, 250)), // off-white
            panel_background_selected: Style::default().bg(Color::Rgb(235, 240, 245)), // light gray
            panel_background_focused: Style::default().bg(Color::Rgb(225, 230, 235)), // slightly darker
            panel_title: Style::default().fg(Color::Rgb(80, 80, 80)),                 // dark gray
            panel_title_selected: Style::default()
                .fg(Color::Rgb(0, 100, 160)) // professional blue
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(0, 100, 160)) // professional blue
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(210, 225, 240)) // soft blue selection
                .fg(Color::Rgb(30, 30, 30)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(190, 210, 230)) // stronger blue selection
                .fg(Color::Rgb(30, 30, 30)),
            status_info: Style::default().fg(Color::Rgb(0, 100, 160)), // professional blue
            status_warning: Style::default().fg(Color::Rgb(180, 120, 0)), // muted amber
            status_success: Style::default().fg(Color::Rgb(40, 130, 80)), // muted green
            status_error: Style::default()
                .fg(Color::Rgb(180, 40, 40)) // muted red
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default()
                .fg(Color::Rgb(0, 0, 160)) // classic dark blue keywords
                .add_modifier(Modifier::BOLD),
            syntax_identifier: Style::default().fg(Color::Rgb(30, 30, 30)), // near-black
            syntax_literal: Style::default().fg(Color::Rgb(0, 110, 60)),    // muted green strings
            syntax_number: Style::default().fg(Color::Rgb(80, 80, 160)), // muted purple-blue numbers
            syntax_comment: Style::default().fg(Color::Rgb(130, 130, 130)), // gray comments
            syntax_operator: Style::default().fg(Color::Rgb(50, 50, 50)), // dark gray operators
            editor_selection: Style::default()
                .bg(Color::Rgb(200, 220, 240)) // soft blue selection
                .fg(Color::Rgb(30, 30, 30)),
        }
    }

    /// Neon-infused futuristic cyberpunk theme
    fn cyberpunk() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(92, 123, 255)), // electric blue
            border_focused: Style::default()
                .fg(Color::Rgb(255, 99, 195)) // neon pink
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(65, 234, 212)) // cyan
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(236, 240, 255)),
            text_muted: Style::default().fg(Color::Rgb(150, 158, 196)),
            accent_primary: Style::default()
                .fg(Color::Rgb(255, 99, 195)) // hot pink
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default()
                .fg(Color::Rgb(65, 234, 212)) // cyan
                .add_modifier(Modifier::BOLD),
            accent_tertiary: Style::default()
                .fg(Color::Rgb(255, 200, 87)) // gold
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(22, 32, 68))
                .fg(Color::Rgb(255, 213, 255)),
            panel_background: Style::default().bg(Color::Rgb(10, 12, 32)), // deep blue-black
            panel_background_selected: Style::default().bg(Color::Rgb(16, 21, 45)),
            panel_background_focused: Style::default().bg(Color::Rgb(24, 31, 63)),
            panel_title: Style::default().fg(Color::Rgb(192, 207, 255)),
            panel_title_selected: Style::default()
                .fg(Color::Rgb(65, 234, 212))
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(255, 99, 195))
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(21, 34, 70))
                .fg(Color::Rgb(224, 236, 255)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(28, 52, 104))
                .fg(Color::Rgb(244, 252, 255)),
            status_info: Style::default().fg(Color::Rgb(65, 234, 212)),
            status_warning: Style::default().fg(Color::Rgb(255, 166, 122)),
            status_success: Style::default().fg(Color::Rgb(120, 255, 195)),
            status_error: Style::default()
                .fg(Color::Rgb(255, 120, 170))
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default().fg(Color::Rgb(255, 99, 195)),
            syntax_identifier: Style::default().fg(Color::Rgb(236, 240, 255)),
            syntax_literal: Style::default().fg(Color::Rgb(255, 200, 87)),
            syntax_number: Style::default().fg(Color::Rgb(102, 216, 255)),
            syntax_comment: Style::default().fg(Color::Rgb(115, 125, 160)),
            syntax_operator: Style::default().fg(Color::Rgb(178, 188, 255)),
            editor_selection: Style::default()
                .bg(Color::Rgb(43, 63, 118))
                .fg(Color::Rgb(255, 255, 255)),
        }
    }

    /// High contrast pure black theme (Neon Tungsten - Cyan & Orange)
    fn midnight() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(100, 100, 100)), // brighter gray for visibility
            border_focused: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // electric cyan
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // electric cyan
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(255, 255, 255)), // pure white
            text_muted: Style::default().fg(Color::Rgb(163, 163, 163)), // neutral-400
            accent_primary: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // electric cyan
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default()
                .fg(Color::Rgb(255, 140, 0)) // dark orange
                .add_modifier(Modifier::BOLD),
            accent_tertiary: Style::default()
                .fg(Color::Rgb(255, 191, 0)) // amber
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(50, 50, 50)) // lighter selection for contrast
                .fg(Color::Rgb(255, 255, 255)),
            panel_background: Style::default().bg(Color::Rgb(0, 0, 0)), // pure black
            panel_background_selected: Style::default().bg(Color::Rgb(20, 20, 20)),
            panel_background_focused: Style::default().bg(Color::Rgb(10, 10, 10)),
            panel_title: Style::default().fg(Color::Rgb(200, 200, 200)),
            panel_title_selected: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // electric cyan
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // electric cyan
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(40, 40, 40))
                .fg(Color::Rgb(255, 255, 255)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(60, 60, 60))
                .fg(Color::Rgb(255, 255, 255)),
            status_info: Style::default().fg(Color::Rgb(0, 255, 255)), // electric cyan
            status_warning: Style::default().fg(Color::Rgb(255, 140, 0)), // dark orange
            status_success: Style::default().fg(Color::Rgb(0, 255, 255)), // electric cyan (matches loading text)
            status_error: Style::default()
                .fg(Color::Rgb(255, 69, 0)) // red-orange
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default().fg(Color::Rgb(255, 140, 0)), // dark orange
            syntax_identifier: Style::default().fg(Color::Rgb(255, 255, 255)), // white
            syntax_literal: Style::default().fg(Color::Rgb(255, 191, 0)), // amber/gold
            syntax_number: Style::default().fg(Color::Rgb(0, 255, 255)),  // electric cyan
            syntax_comment: Style::default().fg(Color::Rgb(128, 128, 128)), // cool grey
            syntax_operator: Style::default().fg(Color::Rgb(255, 255, 255)), // white
            editor_selection: Style::default()
                .bg(Color::Rgb(60, 60, 60))
                .fg(Color::Rgb(255, 255, 255)),
        }
    }

    /// Authentic Nord theme (Sharpened - Deep Ice)
    fn nord() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(135, 170, 200)), // slightly brighter blue
            border_focused: Style::default()
                .fg(Color::Rgb(142, 200, 215)) // brighter cyan
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(142, 200, 215)) // brighter cyan
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(255, 255, 255)), // pure snow white
            text_muted: Style::default().fg(Color::Rgb(220, 225, 235)), // lighter nord4
            accent_primary: Style::default()
                .fg(Color::Rgb(142, 200, 215)) // brighter cyan
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default().fg(Color::Rgb(135, 170, 200)), // brighter blue
            accent_tertiary: Style::default()
                .fg(Color::Rgb(100, 140, 180)) // brighter deep blue
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(60, 70, 85)) // slightly lighter selection
                .fg(Color::Rgb(255, 255, 255)),
            panel_background: Style::default().bg(Color::Rgb(25, 30, 40)), // slightly deeper arctic dark
            panel_background_selected: Style::default().bg(Color::Rgb(40, 45, 55)),
            panel_background_focused: Style::default().bg(Color::Rgb(30, 35, 45)),
            panel_title: Style::default().fg(Color::Rgb(235, 240, 245)), // brighter nord5
            panel_title_selected: Style::default()
                .fg(Color::Rgb(142, 200, 215)) // brighter cyan
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(135, 170, 200)) // brighter blue
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(55, 65, 80))
                .fg(Color::Rgb(255, 255, 255)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(65, 75, 90))
                .fg(Color::Rgb(255, 255, 255)),
            status_info: Style::default().fg(Color::Rgb(142, 200, 215)), // brighter cyan
            status_warning: Style::default().fg(Color::Rgb(240, 210, 150)), // brighter gold
            status_success: Style::default().fg(Color::Rgb(170, 200, 150)), // brighter green
            status_error: Style::default()
                .fg(Color::Rgb(200, 100, 110)) // brighter red
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default().fg(Color::Rgb(142, 200, 215)), // brighter cyan
            syntax_identifier: Style::default().fg(Color::Rgb(245, 250, 255)), // white
            syntax_literal: Style::default().fg(Color::Rgb(170, 200, 150)), // brighter green
            syntax_number: Style::default().fg(Color::Rgb(190, 150, 180)),  // brighter purple
            syntax_comment: Style::default().fg(Color::Rgb(110, 120, 140)), // lighter comment
            syntax_operator: Style::default().fg(Color::Rgb(135, 170, 200)), // brighter blue
            editor_selection: Style::default()
                .bg(Color::Rgb(60, 70, 85))
                .fg(Color::Rgb(255, 255, 255)),
        }
    }

    /// Vibrant Synthwave/Outrun theme (Sharpened)
    fn synthwave() -> Self {
        Self {
            border: Style::default().fg(Color::Rgb(255, 0, 255)), // sharp magenta
            border_focused: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // sharp cyan
                .add_modifier(Modifier::BOLD),
            border_cursor: Style::default()
                .fg(Color::Rgb(255, 255, 0)) // sharp yellow
                .add_modifier(Modifier::BOLD),
            text: Style::default().fg(Color::Rgb(255, 255, 255)), // pure white
            text_muted: Style::default().fg(Color::Rgb(160, 140, 200)), // bright purple-gray
            accent_primary: Style::default()
                .fg(Color::Rgb(255, 0, 255)) // magenta
                .add_modifier(Modifier::BOLD),
            accent_secondary: Style::default()
                .fg(Color::Rgb(0, 255, 255)) // cyan
                .add_modifier(Modifier::BOLD),
            accent_tertiary: Style::default()
                .fg(Color::Rgb(255, 255, 0)) // yellow
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .bg(Color::Rgb(80, 20, 80)) // deep magenta bg
                .fg(Color::Rgb(255, 255, 255)),
            panel_background: Style::default().bg(Color::Rgb(20, 10, 35)), // deep violet-black
            panel_background_selected: Style::default().bg(Color::Rgb(40, 20, 60)),
            panel_background_focused: Style::default().bg(Color::Rgb(30, 15, 50)),
            panel_title: Style::default().fg(Color::Rgb(200, 180, 255)),
            panel_title_selected: Style::default()
                .fg(Color::Rgb(255, 0, 255))
                .add_modifier(Modifier::BOLD),
            panel_title_focused: Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
            grid_selection: Style::default()
                .bg(Color::Rgb(60, 20, 80))
                .fg(Color::Rgb(255, 255, 255)),
            grid_focus_row: Style::default()
                .bg(Color::Rgb(80, 30, 100))
                .fg(Color::Rgb(255, 255, 255)),
            status_info: Style::default().fg(Color::Rgb(0, 255, 255)), // cyan
            status_warning: Style::default().fg(Color::Rgb(255, 150, 0)), // bright orange
            status_success: Style::default().fg(Color::Rgb(0, 255, 100)), // bright green
            status_error: Style::default()
                .fg(Color::Rgb(255, 0, 100)) // bright red-pink
                .add_modifier(Modifier::BOLD),
            syntax_keyword: Style::default().fg(Color::Rgb(255, 0, 255)), // magenta
            syntax_identifier: Style::default().fg(Color::Rgb(255, 255, 255)), // white
            syntax_literal: Style::default().fg(Color::Rgb(255, 255, 0)), // yellow
            syntax_number: Style::default().fg(Color::Rgb(0, 255, 255)),  // cyan
            syntax_comment: Style::default().fg(Color::Rgb(140, 120, 180)), // purple-gray
            syntax_operator: Style::default().fg(Color::Rgb(255, 0, 255)), // magenta
            editor_selection: Style::default()
                .bg(Color::Rgb(80, 20, 80))
                .fg(Color::Rgb(255, 255, 255)),
        }
    }
}
