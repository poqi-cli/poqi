use crate::input::{Action, FocusPanel};
use poqi_catalog::display_identifier;

use super::App;

impl App {
    /// Data-centric actions (auto-fetch, edit, delete) that require panel context.
    pub(super) fn handle_data_action(&mut self, action: Action) {
        match action {
            Action::SelectTop => {
                if self.focus == FocusPanel::Schema && !self.schema.is_table_level() {
                    if self.schema.drill_down() {
                        self.status.info(format!(
                            "Viewing tables in schema: {}",
                            display_identifier(self.schema.current_schema().unwrap_or("unknown"))
                        ));
                        self.auto_fetch_on_table_selection();
                    }
                } else {
                    self.select_top();
                }
            }
            Action::InsertRow => {
                self.status.info("Insert row workflow coming soon");
            }
            Action::UpdateRow => {
                if self.focus == FocusPanel::Results {
                    if self.results.begin_edit() {
                        self.refresh_results_edit_preview();
                    } else if !self.results.can_edit() {
                        self.status.warning("Current result set is read-only");
                    }
                } else {
                    self.status.info("Focus results panel to edit rows");
                }
            }
            Action::DeleteRow => {
                if self.focus == FocusPanel::Results {
                    self.trigger_row_delete();
                } else {
                    self.status.info("Focus results panel to delete rows");
                }
            }
            Action::Refresh => {
                self.status.info("Refresh triggered");
            }
            _ => {}
        }
    }
}
