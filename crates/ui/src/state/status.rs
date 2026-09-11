use std::time::{Duration, Instant};

use ratatui::style::Style;

use crate::theme::Theme;

#[derive(Debug, Clone)]
pub(crate) struct StatusBar {
    pub(crate) message: String,
    level: StatusLevel,
    updated_at: Instant,
    overlay: Option<OverlayNotice>,
    auto_clear_duration: Duration,
    overlay_duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusLevel {
    Info,
    Warning,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct OverlayNotice {
    pub(crate) message: String,
    pub(crate) level: StatusLevel,
    pub(crate) expires_at: Instant,
}

impl StatusBar {
    pub(crate) fn new(auto_clear_duration: Duration, overlay_duration: Duration) -> Self {
        Self {
            message: String::from("Ready"),
            level: StatusLevel::Info,
            updated_at: Instant::now(),
            overlay: None,
            auto_clear_duration,
            overlay_duration,
        }
    }

    pub(crate) fn info(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.level = StatusLevel::Info;
        self.updated_at = Instant::now();
    }

    pub(crate) fn warning(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.level = StatusLevel::Warning;
        self.updated_at = Instant::now();
    }

    pub(crate) fn success(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.level = StatusLevel::Success;
        self.updated_at = Instant::now();
    }

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.level = StatusLevel::Error;
        self.updated_at = Instant::now();
    }

    pub(crate) fn popup(&mut self, msg: impl Into<String>, level: StatusLevel) {
        self.overlay = Some(OverlayNotice {
            message: msg.into(),
            level,
            expires_at: Instant::now() + self.overlay_duration,
        });
    }

    pub(crate) fn popup_error(&mut self, msg: impl Into<String>) {
        self.popup(msg, StatusLevel::Error);
    }

    pub(crate) fn maybe_clear(&mut self) {
        if let Some(overlay) = &self.overlay {
            if overlay.expires_at <= Instant::now() {
                self.overlay = None;
            }
        }
        if self.updated_at.elapsed() > self.auto_clear_duration {
            self.message = String::from("Ready");
            self.level = StatusLevel::Info;
        }
    }

    pub(crate) fn style<'a>(&self, theme: &'a Theme) -> &'a Style {
        match self.level {
            StatusLevel::Info => &theme.status_info,
            StatusLevel::Warning => &theme.status_warning,
            StatusLevel::Success => &theme.status_success,
            StatusLevel::Error => &theme.status_error,
        }
    }

    pub(crate) fn overlay(&self) -> Option<&OverlayNotice> {
        self.overlay.as_ref()
    }

    pub(crate) fn update_durations(&mut self, auto_clear: Duration, overlay: Duration) {
        self.auto_clear_duration = auto_clear;
        self.overlay_duration = overlay;
    }
}
