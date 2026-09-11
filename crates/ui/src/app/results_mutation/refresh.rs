use crate::{
    engine_worker::{EngineCommand, EngineRequestKind},
    state::results_state::RefreshStrategy,
};

use super::{
    super::{requests::PostResultsAction, App},
    context::MutationContext,
};

impl App {
    pub(crate) fn refresh_results_after_mutation(&mut self, context: Option<MutationContext>) {
        match self.results.refresh_strategy() {
            RefreshStrategy::SelectTop { table } => {
                let focus = context.as_ref().and_then(MutationContext::pending_focus);
                let table = table.clone();
                if self.request_select_top_for_table(&table, false) {
                    self.pending_focus = focus;
                } else {
                    self.pending_focus = None;
                }
            }
            RefreshStrategy::RerunSql { sql } => {
                self.pending_focus = context.as_ref().and_then(MutationContext::pending_focus);
                self.request_rerun_sql(sql.to_string());
            }
            RefreshStrategy::RowRefresh { table } => {
                let Some(ctx) = context else {
                    return;
                };
                let (Some(row), Some(row_identity)) = (ctx.row, ctx.row_identity.as_ref()) else {
                    return;
                };
                self.pending_focus = None;
                let table = table.clone();
                self.request_row_refresh(&table, row, row_identity);
            }
            RefreshStrategy::None => {
                self.pending_focus = None;
            }
        }
    }

    pub(crate) fn apply_pending_focus(&mut self) {
        let Some(pending) = self.pending_focus.take() else {
            return;
        };
        let Some(current_table) = self.results.source_table.clone() else {
            return;
        };
        if pending.table != current_table {
            return;
        }
        if let Some((row_idx, _)) = self
            .results
            .row_identities
            .iter()
            .enumerate()
            .find(|(_, identity)| identity.as_ref() == Some(&pending.row_identity))
        {
            let max_col = self.results.headers.len().saturating_sub(1);
            let column = pending.column.min(max_col);
            self.results.set_focus(row_idx, column, false);
        }
    }

    pub(crate) fn request_rerun_sql(&mut self, sql: String) {
        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the results refresh."));
        let request_id = self.next_request_id();
        if self
            .engine_tx
            .send(EngineCommand::RunSql {
                request_id,
                sql,
                kind: EngineRequestKind::RunSql,
            })
            .is_ok()
        {
            self.status.info("Refreshing results…");
            self.start_request(
                request_id,
                EngineRequestKind::RunSql,
                "Auto refresh".to_string(),
                None,
                false,
            );
        } else {
            self.status.warning("Engine unavailable");
            self.pending_focus = None;
        }
    }

    pub(crate) fn request_row_refresh(
        &mut self,
        table: &poqi_catalog::QualifiedRelation,
        row: usize,
        row_identity: &poqi_engine::RowIdentity,
    ) {
        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the row refresh."));
        let request_id = self.next_request_id();
        if self
            .engine_tx
            .send(EngineCommand::Crud {
                request_id,
                action: poqi_engine::CrudAction::RefreshRow {
                    table: table.clone(),
                    row_identity: row_identity.clone(),
                },
                kind: EngineRequestKind::RowRefresh,
            })
            .is_ok()
        {
            self.start_request(
                request_id,
                EngineRequestKind::RowRefresh,
                format!("{} row", table.display_name()),
                Some(PostResultsAction::HydrateRow { row }),
                false,
            );
            self.status.info("Refreshing edited row…");
        } else {
            self.status.warning("Engine unavailable");
        }
    }
}
