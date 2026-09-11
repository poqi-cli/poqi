use std::borrow::Cow;

use ratatui::Frame;

use super::view;

#[derive(Debug)]
pub(super) enum LoadingOutcome {
    Pending,
    Success { detail: Option<String> },
    Failure { detail: String },
}

#[derive(Debug)]
struct Spinner {
    frames: &'static [&'static str],
    index: usize,
}

impl Spinner {
    fn new() -> Self {
        const FRAMES: &[&str] = &["-", "\\", "|", "/"];
        Self {
            frames: FRAMES,
            index: 0,
        }
    }

    fn advance(&mut self) {
        self.index = (self.index + 1) % self.frames.len();
    }

    fn frame(&self) -> &str {
        self.frames[self.index]
    }
}

pub(super) struct LoadingApp {
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) outcome: LoadingOutcome,
    spinner: Spinner,
}

impl LoadingApp {
    pub(super) fn new(title: String, detail: String) -> Self {
        Self {
            title,
            detail,
            outcome: LoadingOutcome::Pending,
            spinner: Spinner::new(),
        }
    }

    pub(super) fn draw(&self, frame: &mut Frame) {
        view::render(self, frame);
    }

    pub(super) fn advance_spinner(&mut self) {
        if matches!(self.outcome, LoadingOutcome::Pending) {
            self.spinner.advance();
        }
    }

    pub(super) fn spinner_frame(&self) -> &str {
        self.spinner.frame()
    }

    pub(super) fn update_detail(&mut self, detail: impl Into<String>) {
        self.detail = detail.into();
    }

    pub(super) fn mark_success(&mut self, detail: Option<String>) {
        self.outcome = LoadingOutcome::Success { detail };
    }

    pub(super) fn mark_failure(&mut self, detail: impl Into<String>) {
        self.outcome = LoadingOutcome::Failure {
            detail: detail.into(),
        };
    }

    pub(super) fn status_line(&self) -> Cow<'_, str> {
        match &self.outcome {
            LoadingOutcome::Pending => {
                Cow::Owned(format!("{}  {}", self.spinner_frame(), self.detail.trim()))
            }
            LoadingOutcome::Success { detail } => Cow::Owned(format!(
                "[ok] {}",
                detail.clone().unwrap_or_else(|| "Done".into())
            )),
            LoadingOutcome::Failure { detail } => Cow::Owned(format!("[err] {detail}")),
        }
    }
}
