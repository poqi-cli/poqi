use std::io::{self, Stdout};

use anyhow::{anyhow, Result};
use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{
        self, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// # Errors
/// Returns error if terminal initialization fails
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    terminal::enable_raw_mode()?;
    let result = (|| {
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        stdout.execute(event::EnableMouseCapture)?;
        stdout.execute(SetCursorStyle::SteadyBar)?;
        let _ = stdout.execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
        ));
        Terminal::new(CrosstermBackend::new(stdout))
    })();
    if result.is_err() {
        restore_after_setup_failure();
    }
    Ok(result?)
}

fn restore_after_setup_failure() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(Show);
    let _ = terminal::disable_raw_mode();
    let _ = stdout.execute(event::DisableMouseCapture);
    let _ = stdout.execute(PopKeyboardEnhancementFlags);
    let _ = stdout.execute(LeaveAlternateScreen);
}

/// # Errors
/// Returns error if terminal shutdown fails
pub fn shutdown_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut first_error = None;
    remember_shutdown_error(&mut first_error, terminal.show_cursor(), "show cursor");
    remember_shutdown_error(
        &mut first_error,
        terminal::disable_raw_mode(),
        "disable raw mode",
    );
    remember_shutdown_error(
        &mut first_error,
        terminal
            .backend_mut()
            .execute(event::DisableMouseCapture)
            .map(|_| ()),
        "disable mouse capture",
    );
    let _ = terminal.backend_mut().execute(PopKeyboardEnhancementFlags);
    remember_shutdown_error(
        &mut first_error,
        terminal
            .backend_mut()
            .execute(LeaveAlternateScreen)
            .map(|_| ()),
        "leave alternate screen",
    );
    first_error.map_or(Ok(()), Err)
}

fn remember_shutdown_error(
    first_error: &mut Option<anyhow::Error>,
    result: io::Result<()>,
    operation: &str,
) {
    if let Err(err) = result {
        tracing::warn!(%err, operation, "terminal restoration step failed");
        if first_error.is_none() {
            *first_error = Some(anyhow!("failed to {operation}: {err}"));
        }
    }
}

/// Owns an initialized terminal and restores it when explicitly closed or dropped.
pub struct TerminalSession {
    terminal: Option<Terminal<CrosstermBackend<Stdout>>>,
}

impl TerminalSession {
    /// Initializes raw mode and the alternate-screen terminal session.
    ///
    /// # Errors
    ///
    /// Returns an error when a terminal setup step fails. Completed setup steps
    /// are rolled back before the error is returned.
    pub fn start() -> Result<Self> {
        setup_terminal().map(|terminal| Self {
            terminal: Some(terminal),
        })
    }

    /// Returns the terminal used for rendering and event-driven updates.
    ///
    /// # Panics
    ///
    /// Panics if the terminal has already been removed for restoration. Public
    /// callers cannot observe that state because [`Self::restore`] consumes the
    /// session.
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        self.terminal
            .as_mut()
            .expect("terminal session must own its terminal until restoration")
    }

    /// Restores the terminal and consumes the session.
    ///
    /// # Errors
    ///
    /// Returns the first error from the terminal restoration sequence after all
    /// restoration steps have been attempted.
    pub fn restore(mut self) -> Result<()> {
        self.terminal.take().map_or(Ok(()), shutdown_terminal)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Some(terminal) = self.terminal.take() {
            if let Err(err) = shutdown_terminal(terminal) {
                tracing::error!(?err, "best-effort terminal restoration failed");
            }
        }
    }
}
