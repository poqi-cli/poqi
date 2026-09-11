use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::state::results_state::plain_text_modifiers;

use super::App;

impl App {
    /// Handle inline editing & delete confirmations inside the results grid.
    pub(super) fn handle_results_key(&mut self, event: KeyEvent) -> bool {
        if event.kind != KeyEventKind::Press {
            return false;
        }
        if !matches!(event.code, KeyCode::Delete)
            && self.results.pending_delete_active()
            && self.results.edit_session().is_none()
        {
            self.results.clear_pending_delete();
            self.clear_editor_overlay();
        }
        match event.code {
            KeyCode::Char('n' | 'N') if event.modifiers == KeyModifiers::CONTROL => {
                if !self.results.set_edit_value_null() {
                    return false;
                }
                self.finalize_results_edit_input();
                self.status
                    .info("Cell set to NULL; Enter saves, Esc cancels");
                true
            }
            KeyCode::Char(ch) if plain_text_modifiers(event.modifiers) => {
                if self.results.edit_session().is_none() {
                    return false;
                }
                if let Some(session) = self.results.edit_session_mut() {
                    session.insert_char(ch);
                }
                self.finalize_results_edit_input();
                true
            }
            KeyCode::Backspace if event.modifiers.is_empty() => {
                if !self.ensure_results_edit_session() {
                    return true;
                }
                if let Some(session) = self.results.edit_session_mut() {
                    session.backspace();
                }
                self.finalize_results_edit_input();
                true
            }
            KeyCode::Enter => {
                if self.results.edit_session().is_some() {
                    self.submit_results_edit();
                    return true;
                }
                if !self.ensure_results_edit_session() {
                    return true;
                }
                self.finalize_results_edit_input();
                true
            }
            KeyCode::Esc => {
                if self.results.edit_session().is_some() {
                    self.cancel_results_edit();
                    return true;
                }
                if self.results.pending_delete_active() {
                    self.results.clear_pending_delete();
                    self.clear_editor_overlay();
                    self.status.info("Delete cancelled");
                    return true;
                }
                false
            }
            KeyCode::Delete if event.modifiers.is_empty() => self.trigger_row_delete(),
            _ => false,
        }
    }

    /// Keep inline edit and pending delete state in sync after typing.
    fn finalize_results_edit_input(&mut self) {
        self.results.clear_pending_delete();
        self.refresh_results_edit_preview();
    }

    fn ensure_results_edit_session(&mut self) -> bool {
        if self.results.edit_session().is_some() {
            return true;
        }
        if self.results.begin_edit() {
            self.status
                .info("Editing cell: Enter saves, Ctrl+N sets NULL, Esc cancels");
            true
        } else {
            if !self.results.can_edit() {
                self.status.warning("Current result set is read-only");
            }
            false
        }
    }
}
