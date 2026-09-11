use std::time::Duration;

pub const DEFAULT_MAIN_TICK_RATE_MS: u64 = 1;
pub const DEFAULT_MENU_TICK_RATE_MS: u64 = 1;
pub const DEFAULT_SELECT_TOP_LIMIT: u32 = 5000;
pub const DEFAULT_FAST_SCROLL_STEP: usize = 10;
pub const DEFAULT_MOUSE_SCROLL_LINES: usize = 3;
pub const DEFAULT_STATUS_EXPIRE_SECS: u64 = 6;
pub const DEFAULT_STATUS_AUTO_CLEAR_SECS: u64 = 6;
pub const DEFAULT_MIN_COLUMN_WIDTH: u16 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiRuntimeSettings {
    main_tick_rate_ms: u64,
    menu_tick_rate_ms: u64,
    fast_scroll_step: usize,
    mouse_scroll_lines: usize,
    select_top_limit: u32,
    status_expire_secs: u64,
    status_auto_clear_secs: u64,
    min_column_width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiRuntimeSettingsParts {
    pub main_tick_rate_ms: u64,
    pub menu_tick_rate_ms: u64,
    pub fast_scroll_step: usize,
    pub mouse_scroll_lines: usize,
    pub select_top_limit: u32,
    pub status_expire_secs: u64,
    pub status_auto_clear_secs: u64,
    pub min_column_width: u16,
}

impl UiRuntimeSettings {
    #[must_use]
    pub const fn from_parts(parts: UiRuntimeSettingsParts) -> Self {
        Self {
            main_tick_rate_ms: parts.main_tick_rate_ms,
            menu_tick_rate_ms: parts.menu_tick_rate_ms,
            fast_scroll_step: parts.fast_scroll_step,
            mouse_scroll_lines: parts.mouse_scroll_lines,
            select_top_limit: parts.select_top_limit,
            status_expire_secs: parts.status_expire_secs,
            status_auto_clear_secs: parts.status_auto_clear_secs,
            min_column_width: parts.min_column_width,
        }
    }

    #[must_use]
    pub fn main_tick_rate(&self) -> Duration {
        Duration::from_millis(self.main_tick_rate_ms.max(1))
    }

    #[must_use]
    pub fn menu_tick_rate(&self) -> Duration {
        Duration::from_millis(self.menu_tick_rate_ms.max(1))
    }

    #[must_use]
    pub fn fast_scroll_step(&self) -> usize {
        self.fast_scroll_step.max(1)
    }

    #[must_use]
    pub fn mouse_scroll_lines(&self) -> usize {
        self.mouse_scroll_lines.max(1)
    }

    #[must_use]
    pub fn select_top_limit(&self) -> u32 {
        self.select_top_limit.max(1)
    }

    #[must_use]
    pub fn status_expire_duration(&self) -> Duration {
        Duration::from_secs(self.status_expire_secs.max(1))
    }

    #[must_use]
    pub fn status_auto_clear_duration(&self) -> Duration {
        Duration::from_secs(self.status_auto_clear_secs.max(1))
    }

    #[must_use]
    pub fn min_column_width(&self) -> u16 {
        self.min_column_width.max(1)
    }
}

impl Default for UiRuntimeSettings {
    fn default() -> Self {
        Self::from_parts(UiRuntimeSettingsParts {
            main_tick_rate_ms: DEFAULT_MAIN_TICK_RATE_MS,
            menu_tick_rate_ms: DEFAULT_MENU_TICK_RATE_MS,
            fast_scroll_step: DEFAULT_FAST_SCROLL_STEP,
            mouse_scroll_lines: DEFAULT_MOUSE_SCROLL_LINES,
            select_top_limit: DEFAULT_SELECT_TOP_LIMIT,
            status_expire_secs: DEFAULT_STATUS_EXPIRE_SECS,
            status_auto_clear_secs: DEFAULT_STATUS_AUTO_CLEAR_SECS,
            min_column_width: DEFAULT_MIN_COLUMN_WIDTH,
        })
    }
}
