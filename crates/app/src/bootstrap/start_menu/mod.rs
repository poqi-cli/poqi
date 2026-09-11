mod form;
mod input;
mod state;
mod view;

use anyhow::Result;
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event},
    ExecutableCommand,
};
use poqi_store::{Store, StoredConnectionProfile};
use poqi_ui::{TerminalSession, TickTimer};
use std::{io, time::Duration};

pub(crate) use form::ProfileForm;
use state::App;
pub(crate) use state::ProfileRecovery;
pub use state::ProfileSelectionResult;

pub fn select_profile_stylish(
    store: Store,
    profiles: Vec<StoredConnectionProfile>,
    initial: usize,
    from_main_ui: bool,
    tick_rate: Duration,
    recovery: Option<ProfileRecovery<'_>>,
) -> Result<ProfileSelectionResult> {
    let mut app = App::new(store, profiles, initial, from_main_ui, recovery);

    let mut terminal = TerminalSession::start()?;
    let paste_guard = BracketedPasteGuard::enable(&mut terminal)?;

    let mut tick_timer = TickTimer::new(tick_rate);

    let run_result = (|| -> Result<()> {
        terminal.terminal_mut().draw(|f| app.draw(f))?;
        loop {
            terminal.terminal_mut().draw(|f| app.draw(f))?;

            if event::poll(tick_timer.timeout())? {
                match event::read()? {
                    Event::Key(key) => match app.handle_key(key) {
                        Ok(true) => break Ok(()),
                        Err(e) => break Err(e),
                        Ok(false) => {}
                    },
                    Event::Mouse(mouse) if app.handle_mouse(mouse) => {
                        break Ok(());
                    }
                    Event::Paste(text) => app.handle_paste(&text),
                    _ => {}
                }
            }

            if tick_timer.is_elapsed() {
                tick_timer.reset();
            }
        }
    })();

    let shutdown_result = terminal.restore();
    let paste_result = paste_guard.disable();

    run_result?;
    paste_result?;
    shutdown_result?;

    let default_result = app
        .profiles
        .get(app.selected)
        .cloned()
        .map_or(ProfileSelectionResult::GenerateTestData, |profile| {
            ProfileSelectionResult::Selected { profile }
        });

    Ok(app.result.unwrap_or(default_result))
}

struct BracketedPasteGuard {
    enabled: bool,
}

impl BracketedPasteGuard {
    fn enable(terminal: &mut TerminalSession) -> Result<Self> {
        terminal
            .terminal_mut()
            .backend_mut()
            .execute(EnableBracketedPaste)?;
        Ok(Self { enabled: true })
    }

    fn disable(mut self) -> Result<()> {
        self.enabled = false;
        io::stdout().execute(DisableBracketedPaste)?;
        Ok(())
    }
}

impl Drop for BracketedPasteGuard {
    fn drop(&mut self) {
        if self.enabled {
            let _ = io::stdout().execute(DisableBracketedPaste);
        }
    }
}
