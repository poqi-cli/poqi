use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

use poqi_engine::{CrudAction, QueryResult, ResultOrigin};
use tokio::sync::mpsc::error::TryRecvError;

use crate::{
    app::{requests::PostResultsAction, ActiveSemanticJob, App},
    engine_worker::{EngineCommand, EngineRequestKind},
    semantic_worker::{SemanticCommand, SemanticResponse, SemanticStats},
};

impl App {
    pub(crate) fn trigger_semantic_search(&mut self) {
        if self.semantic.is_disabled() {
            if let Some(reason) = &self.semantic_disabled_reason {
                self.status.warning(reason.clone());
            } else {
                self.status.warning("Semantic search is disabled");
            }
            return;
        }

        let query_text = self.semantic.query();
        let query = query_text.trim().to_string();
        if query.is_empty() {
            self.status.warning("Enter a natural language prompt first");
            return;
        }

        let Some(table) = self.schema.selected_table_name() else {
            self.status.warning("Select a table to filter first");
            return;
        };

        self.cancel_semantic_pipeline(Some("Semantic search cancelled before starting a new run."));

        if self.semantic_tx.is_none() {
            if self.semantic_events.is_some() {
                self.semantic
                    .set_loading("Semantic model still downloading.");
                self.status
                    .info("Semantic model is still downloading; please wait");
            } else {
                self.semantic
                    .set_error("Semantic model unavailable on this system");
                self.status
                    .warning("Semantic search is unavailable on this system");
            }
            return;
        }

        let request_id = self.next_request_id();
        let limit = self.ui_settings.select_top_limit();
        let table_label = table.display_name();
        let Some(relation_kind) = self.relation_kind_for_table(&table) else {
            self.semantic
                .set_error(format!("Missing catalog metadata for {table_label}"));
            self.status
                .warning(format!("Missing catalog metadata for {table_label}"));
            return;
        };
        let action = CrudAction::SelectTop {
            table: table.clone(),
            limit,
            relation_kind,
        };
        let send_result = self.engine_tx.send(EngineCommand::Crud {
            request_id,
            action,
            kind: EngineRequestKind::SemanticSearch,
        });
        if send_result.is_ok() {
            self.semantic
                .set_loading(format!("Fetching rows from {table_label}."));
            self.status
                .info(format!("Semantic search: fetching {table_label}"));
            self.start_request(
                request_id,
                EngineRequestKind::SemanticSearch,
                table_label,
                Some(PostResultsAction::Semantic { query }),
                false,
            );
        } else {
            self.status.warning("Engine unavailable");
        }
    }

    pub(crate) fn cancel_semantic_pipeline(&mut self, reason: Option<&str>) -> bool {
        let mut cancelled = false;
        let mut cancelled_table = None;
        let mut attempted_engine_cancel = false;
        if let Some(active) = self.active_semantic.take() {
            active.cancel_flag.store(true, Ordering::Relaxed);
            cancelled = true;
            cancelled_table = active.table;
        }

        let mut engine_cancel_failed = false;
        if self
            .requests
            .active()
            .is_some_and(|req| req.kind == EngineRequestKind::SemanticSearch)
        {
            attempted_engine_cancel = true;
            if self.engine_tx.send(EngineCommand::CancelActive).is_ok() {
                cancelled = true;
            } else {
                engine_cancel_failed = true;
            }
        }

        if cancelled || attempted_engine_cancel {
            self.semantic.set_idle();
            let message = reason
                .map(str::to_owned)
                .or_else(|| {
                    cancelled_table.map(|table| format!("Semantic search for {table} cancelled"))
                })
                .unwrap_or_else(|| "Semantic search cancelled".to_string());
            self.status.info(message);
            if engine_cancel_failed {
                self.status.warning("Engine unavailable");
            }
        }

        cancelled || attempted_engine_cancel
    }

    pub(crate) fn poll_semantic(&mut self) {
        loop {
            let reaction = {
                let Some(rx) = self.semantic_rx.as_mut() else {
                    return;
                };
                match rx.try_recv() {
                    Ok(response) => SemanticPoll::Response(Box::new(response)),
                    Err(TryRecvError::Empty) => SemanticPoll::Idle,
                    Err(TryRecvError::Disconnected) => SemanticPoll::Disconnected,
                }
            };

            match reaction {
                SemanticPoll::Response(response) => self.handle_semantic_response(*response),
                SemanticPoll::Idle => break,
                SemanticPoll::Disconnected => {
                    self.semantic_rx = None;
                    self.semantic
                        .set_error("Semantic worker disconnected unexpectedly");
                    break;
                }
            }
        }
    }

    fn handle_semantic_response(&mut self, response: SemanticResponse) {
        let Some(active) = self.active_semantic.clone() else {
            return;
        };
        match response {
            SemanticResponse::Success {
                request_id,
                result,
                stats,
            } => {
                if request_id != active.id {
                    return;
                }
                self.apply_semantic_result(*result, &stats);
            }
            SemanticResponse::Progress {
                request_id,
                message,
            } => {
                if request_id != active.id {
                    return;
                }
                self.semantic.set_loading(message);
            }
            SemanticResponse::Error {
                request_id,
                message,
            } => {
                if request_id != active.id {
                    return;
                }
                self.semantic.set_error(message.clone());
                self.status
                    .error(format!("Semantic search failed: {message}"));
                self.active_semantic = None;
            }
            SemanticResponse::Canceled { request_id } => {
                if request_id != active.id {
                    return;
                }
                self.semantic.set_idle();
                self.active_semantic = None;
            }
        }
    }

    fn apply_semantic_result(&mut self, result: QueryResult, stats: &SemanticStats) {
        let QueryResult {
            columns,
            rows,
            metadata,
        } = result;
        self.results.set_data(columns, rows, metadata);
        self.apply_pending_focus();
        let threshold_enabled = self.semantic_options.threshold.is_some();
        let mut ready = semantic_ready_summary(stats, threshold_enabled);
        if let Some(note) = &stats.fallback_notice {
            ready.push_str(" | ");
            ready.push_str(note);
            self.status.warning(note.clone());
        }
        self.semantic.set_ready(ready);
        let total_embeddings = stats.cache_hits + stats.cache_misses;
        let cache_note = if total_embeddings > 0 {
            format!(", cache hit {}/{}", stats.cache_hits, total_embeddings)
        } else {
            String::new()
        };
        if stats.matched_rows == 0 {
            let outcome = if threshold_enabled {
                "found no rows meeting the score threshold"
            } else {
                "had no rows to rank"
            };
            self.status.info(format!(
                "Semantic search {outcome} in {} ms{cache_note}",
                stats.elapsed_ms
            ));
        } else {
            self.status.success(format!(
                "Semantic search matched {} row(s) in {} ms (top {:.2}{cache_note})",
                stats.matched_rows, stats.elapsed_ms, stats.top_score
            ));
        }
        self.active_semantic = None;
    }

    pub(crate) fn dispatch_semantic_job(&mut self, result: QueryResult, query: String) {
        let Some(tx) = self.semantic_tx.clone() else {
            self.semantic
                .set_error("Semantic model unavailable on this system");
            return;
        };

        let request_id = self.next_semantic_request_id();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let table = Self::semantic_table_name(&result);
        self.active_semantic = Some(ActiveSemanticJob {
            id: request_id,
            table,
            cancel_flag: Arc::clone(&cancel_flag),
        });

        if tx
            .send(SemanticCommand::RankRows {
                request_id,
                query,
                rows: result,
                title_column: self.semantic_title_column.clone(),
                options: self.semantic_options,
                cancel: cancel_flag,
            })
            .is_err()
        {
            self.semantic
                .set_error("Semantic worker unavailable (send failed)");
            self.active_semantic = None;
            return;
        }
        self.semantic
            .set_loading("Ranking rows via semantic embeddings.");
    }

    fn next_semantic_request_id(&mut self) -> u64 {
        let id = self.next_semantic_request_id;
        self.next_semantic_request_id = self.next_semantic_request_id.wrapping_add(1);
        if self.next_semantic_request_id == 0 {
            self.next_semantic_request_id = 1;
        }
        id
    }

    fn semantic_table_name(result: &QueryResult) -> Option<String> {
        match &result.metadata.origin {
            ResultOrigin::SelectTop { table, .. } => Some(table.display_name()),
            ResultOrigin::RunSql { refresh, .. } => Some(refresh.table().display_name()),
            ResultOrigin::Unknown => result
                .metadata
                .source_table
                .as_ref()
                .map(poqi_catalog::QualifiedRelation::display_name),
        }
    }
}

fn semantic_ready_summary(stats: &SemanticStats, threshold_enabled: bool) -> String {
    if stats.matched_rows == 0 {
        let outcome = if threshold_enabled {
            "No rows met the semantic score threshold"
        } else {
            "No rows were available for semantic ranking"
        };
        format!("{outcome} | {}", stats.backend_label)
    } else {
        format!(
            "Matched {} row(s) | top score {:.2} | {}",
            stats.matched_rows, stats.top_score, stats.backend_label
        )
    }
}

enum SemanticPoll {
    Response(Box<SemanticResponse>),
    Idle,
    Disconnected,
}

#[cfg(test)]
mod tests {
    use super::semantic_ready_summary;
    use crate::semantic_worker::SemanticStats;

    #[test]
    fn zero_match_summary_does_not_claim_a_top_score() {
        let summary = semantic_ready_summary(
            &SemanticStats {
                matched_rows: 0,
                top_score: 0.0,
                fallback_notice: None,
                backend_label: "mock".to_string(),
                elapsed_ms: 1,
                cache_hits: 0,
                cache_misses: 2,
            },
            true,
        );

        assert_eq!(summary, "No rows met the semantic score threshold | mock");
        assert!(!summary.contains("top score"));
    }
}
