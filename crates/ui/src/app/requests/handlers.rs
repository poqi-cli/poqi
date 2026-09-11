use poqi_catalog::CatalogSnapshot;
use poqi_engine::QueryResult;

use crate::engine_worker::{EngineRequestKind, EngineResponse};

use super::super::{App, MutationContext};
use super::PostResultsAction;

impl App {
    pub(crate) fn handle_engine_response(&mut self, response: EngineResponse) {
        match response {
            EngineResponse::Success {
                request_id,
                kind,
                result,
            } => {
                if self.try_handle_success(request_id, kind, *result) {
                    return;
                }
                self.handle_inactive_success(request_id, kind);
            }
            EngineResponse::CatalogRefreshed {
                request_id,
                snapshot,
            } => {
                if self.try_handle_catalog_refresh(request_id, &snapshot) {
                    return;
                }
                if self.catalog_refresh.on_canceled(request_id) {
                    self.status
                        .warning("Previous catalog refresh completed; a fresh refresh was queued");
                } else {
                    tracing::debug!(request_id, "ignoring catalog refresh for inactive request");
                }
            }
            EngineResponse::Error {
                request_id,
                kind,
                message,
            } => {
                if self.try_handle_error(request_id, kind, &message) {
                    return;
                }
                self.handle_inactive_error(request_id, kind, &message);
            }
            EngineResponse::Canceled {
                request_id, kind, ..
            } => {
                if self
                    .requests
                    .take_if(|req| req.id == request_id && req.kind == kind)
                    .is_some()
                {
                    self.clear_editor_overlay();
                    self.results.clear_pending_delete();
                    self.status.info("Operation canceled");
                }
                self.catalog_refresh.on_canceled(request_id);
                tracing::debug!(
                    request_id,
                    ?kind,
                    "requested operation cancellation confirmed"
                );
            }
        }
    }

    fn handle_inactive_success(&mut self, request_id: u64, kind: EngineRequestKind) {
        tracing::info!(request_id, ?kind, "previous request completed successfully");
        match kind {
            EngineRequestKind::RunSql => {
                // The superseded request's tracker (including its parsed DDL flag) is gone.
                // Refresh conservatively because the completed SQL may have changed catalog state.
                self.catalog_refresh.mark_pending();
                self.status.warning(
                    "Previous SQL request completed successfully; its results were not displayed and a catalog refresh was queued",
                );
            }
            EngineRequestKind::UpdateCell => {
                self.status.warning(
                    "Previous row update completed successfully; refresh results to confirm the committed change",
                );
            }
            EngineRequestKind::DeleteRow => {
                self.status.warning(
                    "Previous row deletion completed successfully; refresh results to confirm the committed change",
                );
            }
            EngineRequestKind::CatalogRefresh => {
                if self.catalog_refresh.on_canceled(request_id) {
                    self.status
                        .warning("Previous catalog refresh completed; a fresh refresh was queued");
                }
            }
            EngineRequestKind::SelectTop
            | EngineRequestKind::RowRefresh
            | EngineRequestKind::SemanticSearch => {
                tracing::debug!(
                    request_id,
                    ?kind,
                    "ignoring engine success for inactive request"
                );
            }
        }
    }

    fn handle_inactive_error(&mut self, request_id: u64, kind: EngineRequestKind, message: &str) {
        tracing::warn!(request_id, ?kind, %message, "previous request failed");
        if matches!(kind, EngineRequestKind::RunSql) {
            // An unknown SQL outcome may include catalog changes, so reconcile once the
            // newer request has finished.
            self.catalog_refresh.mark_pending();
        }
        if matches!(
            kind,
            EngineRequestKind::RunSql
                | EngineRequestKind::UpdateCell
                | EngineRequestKind::DeleteRow
        ) {
            let label = match kind {
                EngineRequestKind::RunSql => "SQL request",
                EngineRequestKind::UpdateCell => "row update",
                EngineRequestKind::DeleteRow => "row deletion",
                _ => unreachable!("matched request kinds are exhaustive"),
            };
            let summary = format!("Previous {label} failed: {message}");
            self.status.error(summary.clone());
            self.status.popup_error(summary);
            return;
        }
        if matches!(kind, EngineRequestKind::CatalogRefresh)
            && self.catalog_refresh.on_canceled(request_id)
        {
            self.status
                .warning("Previous catalog refresh failed; retry queued");
        }
    }

    fn try_handle_success(
        &mut self,
        request_id: u64,
        kind: EngineRequestKind,
        result: QueryResult,
    ) -> bool {
        let Some(active) = self
            .requests
            .take_if(|req| req.id == request_id && req.kind == kind)
        else {
            return false;
        };

        match kind {
            EngineRequestKind::RunSql | EngineRequestKind::SelectTop => {
                self.handle_query_success(kind, active, result);
            }
            EngineRequestKind::SemanticSearch => {
                self.handle_semantic_success(active, result);
            }
            EngineRequestKind::UpdateCell => {
                self.handle_update_success(active, result);
            }
            EngineRequestKind::DeleteRow => {
                self.handle_delete_success(&active, result);
            }
            EngineRequestKind::RowRefresh => {
                self.handle_row_refresh_success(&active, result);
            }
            EngineRequestKind::CatalogRefresh => {
                tracing::warn!(
                    request_id,
                    "received query result for catalog refresh; dropping result rows"
                );
            }
        }

        true
    }

    fn handle_query_success(
        &mut self,
        kind: EngineRequestKind,
        active: ActiveRequest,
        result: QueryResult,
    ) {
        let ActiveRequest {
            label,
            started_at,
            refresh_catalog_on_success,
            ..
        } = active;
        let QueryResult {
            columns,
            rows,
            metadata,
        } = result;
        self.results.set_data(columns, rows, metadata);
        self.apply_pending_focus();
        self.clear_editor_overlay();

        let elapsed_ms = started_at.elapsed().as_millis();
        let row_count = self.results.row_count();
        let message = if matches!(kind, EngineRequestKind::SelectTop) {
            format!("Fetched {row_count} row(s) from {label} in {elapsed_ms} ms")
        } else {
            format!("Query finished with {row_count} row(s) in {elapsed_ms} ms")
        };
        self.status.success(message);
        if refresh_catalog_on_success {
            self.catalog_refresh.mark_pending();
        }
    }

    fn handle_semantic_success(&mut self, active: ActiveRequest, result: QueryResult) {
        if let Some(PostResultsAction::Semantic { query }) = active.follow_up {
            self.dispatch_semantic_job(result, query);
        } else {
            self.semantic
                .set_error("Semantic search follow-up metadata missing");
        }
    }

    fn handle_update_success(&mut self, active: ActiveRequest, result: QueryResult) {
        let mut context = None;
        if let Some(PostResultsAction::ApplyCellEdit { row, column, value }) = active.follow_up {
            self.results.apply_cell_update(row, column, value);
            if let Some(new_identity) = result.metadata.row_identities.into_iter().flatten().next()
            {
                self.results.update_row_identity(row, new_identity.clone());
                if let Some(table) = self.results.source_table.clone() {
                    context = Some(MutationContext {
                        table,
                        row_identity: Some(new_identity),
                        row: Some(row),
                        column: Some(column),
                    });
                }
            }
        }
        self.clear_editor_overlay();
        self.status.success(format!("{} applied", active.label));
        self.refresh_results_after_mutation(context);
    }

    fn handle_delete_success(&mut self, active: &ActiveRequest, result: QueryResult) {
        drop(result);
        if let Some(PostResultsAction::DeleteRow { row }) = active.follow_up {
            self.results.remove_row(row);
        }
        self.clear_editor_overlay();
        self.status
            .success(format!("Deleted row from {}", active.label));
        let context = self
            .results
            .source_table
            .clone()
            .map(|table| MutationContext {
                table,
                row_identity: None,
                row: None,
                column: None,
            });
        self.refresh_results_after_mutation(context);
    }

    fn handle_row_refresh_success(&mut self, active: &ActiveRequest, result: QueryResult) {
        if let Some(PostResultsAction::HydrateRow { row }) = active.follow_up {
            if let Some(first_row) = result.rows.first() {
                let row_identity = result.metadata.row_identities.into_iter().flatten().next();
                let nulls = result
                    .metadata
                    .null_cells
                    .first()
                    .map_or(&[][..], Vec::as_slice);
                self.results
                    .hydrate_row(row, &result.columns, first_row, nulls, row_identity);
                self.status.success(format!("{} refreshed", active.label));
            } else {
                self.status.warning(format!(
                    "{label} refresh returned no data",
                    label = active.label
                ));
            }
        }
    }

    fn try_handle_error(
        &mut self,
        request_id: u64,
        kind: EngineRequestKind,
        message: &str,
    ) -> bool {
        let Some(active) = self
            .requests
            .take_if(|req| req.id == request_id && req.kind == kind)
        else {
            return false;
        };

        if matches!(kind, EngineRequestKind::RunSql)
            && (active.refresh_catalog_on_success || message.contains("outcome is unknown"))
        {
            self.catalog_refresh.mark_pending();
        }
        let summary = format!("{} failed: {message}", active.label);
        self.status.error(summary.clone());
        self.status.popup_error(summary);
        if matches!(
            kind,
            EngineRequestKind::UpdateCell | EngineRequestKind::DeleteRow
        ) {
            self.clear_editor_overlay();
            self.results.clear_pending_delete();
        }
        if matches!(kind, EngineRequestKind::SemanticSearch) {
            self.semantic.set_error(message.to_string());
        }
        if matches!(kind, EngineRequestKind::CatalogRefresh) {
            self.catalog_refresh.on_failure(std::time::Instant::now());
        }
        true
    }

    fn try_handle_catalog_refresh(&mut self, request_id: u64, snapshot: &CatalogSnapshot) -> bool {
        let Some(active) = self
            .requests
            .take_if(|req| req.id == request_id && req.kind == EngineRequestKind::CatalogRefresh)
        else {
            return false;
        };

        self.apply_catalog_snapshot(snapshot);
        self.catalog_refresh.on_success();
        self.status.success(format!("{} refreshed", active.label));
        true
    }
}
use super::super::requests::tracker::ActiveRequest;
