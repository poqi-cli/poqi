use crate::{
    engine_worker::EngineCommand,
    input::{Action, FocusPanel},
};
use poqi_catalog::display_identifier;

use super::App;

impl App {
    /// Handle generic UI flow actions (open/back/zoom resets).
    pub(super) fn handle_flow_action(&mut self, action: Action) {
        match action {
            Action::Open => self.on_open(),
            Action::Back => self.on_back(),
            Action::TableDetail => self.on_table_detail(),
            _ => {}
        }
    }

    pub(super) fn on_open(&mut self) {
        match self.focus {
            FocusPanel::Schema => {
                if self.schema.is_table_level() && self.toggle_table_detail_for_selection() {
                    // Detail panel opened; nothing else to do.
                } else if self.schema.drill_down() {
                    self.status.info(format!(
                        "Viewing tables in schema: {}",
                        display_identifier(self.schema.current_schema().unwrap_or("unknown"))
                    ));
                    self.auto_fetch_on_table_selection();
                } else {
                    self.focus_window(FocusPanel::Editor);
                    if let Some(table) = self.schema.selected_table_name() {
                        self.status
                            .info(format!("Loaded template for {}", table.display_name()));
                    }
                }
            }
            FocusPanel::Editor => {
                self.focus_window(FocusPanel::Results);
                self.status.info("Moved to results panel");
            }
            FocusPanel::SemanticSearch => {
                self.status
                    .info("Semantic search: type a prompt and press Enter");
            }
            FocusPanel::Results => self.status.info("Row activated (stub)"),
            FocusPanel::Status => self.open_settings_modal(),
        }
    }

    fn on_back(&mut self) {
        if self.table_detail.is_open() {
            self.table_detail.close();
            self.status.info("Closed table detail");
            return;
        }
        if self.request_cancel() {
            return;
        }
        if self.zoomed_panel.is_some() {
            self.zoomed_panel = None;
            return;
        }
        if self.focus == FocusPanel::Schema && self.schema.go_back() {
            self.status.info("Returned to schema list");
            return;
        }
        self.enter_window_layer();
    }

    pub(super) fn request_cancel(&mut self) -> bool {
        let mut canceled = false;
        let semantic_canceled = self.cancel_semantic_pipeline(Some("Semantic search cancelled"));
        if self.requests.is_active() && !semantic_canceled {
            if self.engine_tx.send(EngineCommand::CancelActive).is_ok() {
                self.status.warning("Cancellation requested");
            } else {
                self.status.warning("Engine unavailable");
            }
            canceled = true;
        }

        canceled |= semantic_canceled;

        canceled
    }

    pub(super) fn on_table_detail(&mut self) {
        if self.focus != FocusPanel::Schema {
            return;
        }
        if self.schema.is_table_level() {
            self.toggle_table_detail_for_selection();
            return;
        }
        if self.schema.drill_down() {
            self.status.info(format!(
                "Viewing tables in schema: {}",
                display_identifier(self.schema.current_schema().unwrap_or("unknown"))
            ));
            self.auto_fetch_on_table_selection();
        }
    }

    fn toggle_table_detail_for_selection(&mut self) -> bool {
        let Some(table) = self.schema.selected_table_name() else {
            self.status.warning("Select a table to view details");
            return false;
        };
        if self
            .table_detail
            .current()
            .is_some_and(|detail| detail.table == table.display_name())
        {
            self.table_detail.close();
            self.status.info("Closed table detail");
            return true;
        }
        if self.table_detail.open_for(&table).is_some() {
            self.status
                .info(format!("Columns for {}", table.display_name()));
            return true;
        }
        self.status
            .warning(format!("No metadata for {}", table.display_name()));
        false
    }
}
