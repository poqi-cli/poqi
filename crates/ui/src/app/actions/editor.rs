use crate::input::{Action, FocusPanel};

use super::App;

impl App {
    /// Handle clipboard + execution actions originating from the editor or results panels.
    pub(super) fn handle_editor_action(&mut self, action: Action) {
        match action {
            Action::Copy => self.copy_selection(),
            Action::Export => self.status.success("Export CSV queued (stub)"),
            Action::Run => self.run_query(),
            Action::Cancel => {
                if self.request_cancel() {
                    // cancellation handled
                } else if matches!(self.focus, FocusPanel::Results) {
                    self.copy_selection();
                } else {
                    self.status.warning("No running query to cancel");
                }
            }
            _ => {}
        }
    }
}
