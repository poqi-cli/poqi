use poqi_engine::CrudAction;

use crate::engine_worker::{EngineCommand, EngineRequestKind};

use super::super::{requests::PostResultsAction, App};
use crate::state::results_state::EditCompletion;

impl App {
    pub(crate) fn refresh_results_edit_preview(&mut self) {
        let Some(session) = self.results.edit_session() else {
            return;
        };
        let Some(column_name) = self
            .results
            .source_columns
            .get(session.column)
            .and_then(Option::as_deref)
        else {
            return;
        };
        let Some((table, row_identity)) = self.results.focused_row_identity() else {
            return;
        };
        let table_sql = table.quoted();
        let column_sql = Self::quote_identifier(column_name);
        let value_sql = if session.is_null() {
            "NULL".to_string()
        } else {
            Self::literal(session.buffer())
        };
        let predicate = Self::row_identity_preview(row_identity);
        let preview =
            format!("UPDATE {table_sql}\nSET {column_sql} = {value_sql}\nWHERE {predicate}");
        self.show_editor_overlay(&preview);
    }

    pub(crate) fn submit_results_edit(&mut self) -> bool {
        if self
            .requests
            .active()
            .is_some_and(|req| matches!(req.kind, EngineRequestKind::UpdateCell))
        {
            self.status
                .warning("Finish the previous edit before starting another");
            return false;
        }

        let Some(completion) = self.results.finish_edit() else {
            return false;
        };
        let pending = match completion {
            EditCompletion::Unchanged => {
                self.clear_editor_overlay();
                self.status.info("Cell unchanged");
                return true;
            }
            EditCompletion::Changed(pending) => pending,
        };
        let Some(table) = self.results.source_table.clone() else {
            self.status.warning("Missing table context for edit");
            return false;
        };
        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the row update."));
        let request_id = self.next_request_id();
        let action = CrudAction::UpdateCell {
            table: table.clone(),
            column: pending.column_name.clone(),
            new_value: pending.new_value.clone(),
            row_identity: pending.row_identity.clone(),
            column_type: pending.column_type.clone(),
        };
        if self
            .engine_tx
            .send(EngineCommand::Crud {
                request_id,
                action,
                kind: EngineRequestKind::UpdateCell,
            })
            .is_ok()
        {
            self.start_request(
                request_id,
                EngineRequestKind::UpdateCell,
                format!(
                    "{}.{}",
                    table.display_name(),
                    poqi_catalog::display_identifier(&pending.column_name)
                ),
                Some(PostResultsAction::ApplyCellEdit {
                    row: pending.row,
                    column: pending.column,
                    value: pending.new_value,
                }),
                false,
            );
            self.status.info("Update queued");
            true
        } else {
            self.status.warning("Engine unavailable");
            self.clear_editor_overlay();
            false
        }
    }

    pub(crate) fn cancel_results_edit(&mut self) {
        if self.results.edit_session().is_some() {
            self.results.cancel_edit();
            self.clear_editor_overlay();
            self.status.info("Edit cancelled");
        }
    }

    pub(crate) fn trigger_row_delete(&mut self) -> bool {
        if !self.results.can_edit() {
            self.status.warning("Current result set is read-only");
            return true;
        }
        if !self.results.pending_delete_active() {
            if let Some((table, row_identity)) = self.results.focused_row_identity() {
                let preview = format!(
                    "DELETE FROM {table}\nWHERE {predicate}",
                    table = table.quoted(),
                    predicate = Self::row_identity_preview(row_identity)
                );
                self.results.arm_pending_delete();
                self.show_editor_overlay(&preview);
                self.status
                    .warning("Press Delete again to confirm row removal");
                return true;
            }
            self.status.warning("No row selected for deletion");
            return true;
        }
        self.submit_row_delete()
    }

    fn submit_row_delete(&mut self) -> bool {
        if self
            .requests
            .active()
            .is_some_and(|req| matches!(req.kind, EngineRequestKind::DeleteRow))
        {
            self.status
                .warning("Finish the previous delete before starting another");
            return false;
        }

        let row = self.results.focus_cell.0;
        let Some((table, row_identity)) = self
            .results
            .focused_row_identity()
            .map(|(table, identity)| (table.clone(), identity.clone()))
        else {
            self.status.warning("No row selected for deletion");
            return false;
        };
        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the row deletion."));
        let request_id = self.next_request_id();
        let action = CrudAction::DeleteRow {
            table: table.clone(),
            row_identity,
        };
        if self
            .engine_tx
            .send(EngineCommand::Crud {
                request_id,
                action,
                kind: EngineRequestKind::DeleteRow,
            })
            .is_ok()
        {
            self.results.clear_pending_delete();
            self.start_request(
                request_id,
                EngineRequestKind::DeleteRow,
                table.display_name(),
                Some(PostResultsAction::DeleteRow { row }),
                false,
            );
            self.status.warning("Delete requested");
            true
        } else {
            self.status.warning("Engine unavailable");
            self.results.clear_pending_delete();
            self.clear_editor_overlay();
            false
        }
    }
}
