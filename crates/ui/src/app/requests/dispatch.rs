use poqi_catalog::QualifiedRelation;
use poqi_catalog::RelationKind;
use poqi_engine::CrudAction;

use crate::engine_worker::{EngineCommand, EngineRequestKind};

use super::super::{catalog_refresh::mutates_catalog, App};
use super::tracker::{ActiveRequest, PostResultsAction};

impl App {
    pub(crate) fn next_request_id(&mut self) -> u64 {
        self.requests.next_id()
    }

    pub(crate) fn start_request(
        &mut self,
        id: u64,
        kind: EngineRequestKind,
        label: String,
        follow_up: Option<PostResultsAction>,
        refresh_catalog_on_success: bool,
    ) {
        self.requests.start(ActiveRequest::new(
            id,
            kind,
            label,
            follow_up,
            refresh_catalog_on_success,
        ));
    }

    pub(crate) fn run_query(&mut self) {
        let sql = self.editor.text();
        if sql.trim().is_empty() {
            self.status.warning("Nothing to run");
            return;
        }

        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the new SQL query."));
        let request_id = self.next_request_id();
        if self
            .engine_tx
            .send(EngineCommand::RunSql {
                request_id,
                sql: sql.clone(),
                kind: EngineRequestKind::RunSql,
            })
            .is_ok()
        {
            let preview = Self::preview_sql(&sql);
            self.status.info(format!("Running query… ({preview})"));
            self.start_request(
                request_id,
                EngineRequestKind::RunSql,
                preview,
                None,
                mutates_catalog(&sql),
            );
        } else {
            self.status.warning("Engine unavailable");
        }
    }

    pub(crate) fn select_top(&mut self) {
        let Some(table) = self.schema.selected_table_name() else {
            self.status.warning("No table selected");
            return;
        };

        self.request_select_top_for_table(&table, true);
    }

    pub(crate) fn auto_fetch_on_table_selection(&mut self) {
        let selected_table = self.schema.selected_table_name();
        self.table_detail.follow_selection(selected_table.as_ref());
        if self.schema.selected_table_requires_explicit_preview() {
            if let Some(table) = selected_table {
                self.status.warning(format!(
                    "Automatic preview paused for {}; use Select Top (default: Shift+F) to fetch it",
                    table.display_name()
                ));
            }
            return;
        }
        let should_auto_fetch = self.schema.should_auto_fetch();
        if should_auto_fetch {
            self.cancel_semantic_pipeline(Some(
                "Semantic search cancelled after switching tables.",
            ));
        }
        if should_auto_fetch {
            self.select_top();
        }
    }

    pub(crate) fn copy_selection(&mut self) {
        if let Some(((row_start, col_start), (row_end, col_end))) = self.results.selection_bounds()
        {
            let mut preview = Vec::new();
            for row in row_start..=row_end {
                let line = (col_start..=col_end)
                    .filter_map(|col| self.results.rows.get(row).and_then(|r| r.get(col)).cloned())
                    .collect::<Vec<_>>()
                    .join("\t");
                preview.push(line);
            }
            self.status.success(format!(
                "Copied {} row(s) to clipboard (mock)",
                preview.len()
            ));
        } else if let Some(value) = self.results.current_value() {
            self.status.success(format!("Copied cell '{value}' (mock)"));
        }
    }

    pub(crate) fn poll_engine(&mut self) {
        while let Ok(response) = self.engine_rx.try_recv() {
            self.handle_engine_response(response);
        }
    }

    pub(crate) fn request_select_top_for_table(
        &mut self,
        table: &QualifiedRelation,
        update_editor: bool,
    ) -> bool {
        self.cancel_semantic_pipeline(Some("Semantic search cancelled by the table preview."));
        let Some(relation_kind) = self.relation_kind_for_table(table) else {
            self.status.warning(format!(
                "Missing catalog metadata for {}",
                table.display_name()
            ));
            return false;
        };
        let limit = self.ui_settings.select_top_limit();
        if update_editor {
            let sql = format!("SELECT * FROM {} LIMIT {limit};", table.quoted());
            self.editor.set_text(&sql);
            self.editor.set_cursor(0, 0);
            self.editor.scroll_row = 0;
            self.clamp_editor_scroll_to_cursor();
        }

        let request_id = self.next_request_id();
        let action = CrudAction::SelectTop {
            table: table.to_owned(),
            limit,
            relation_kind,
        };
        if self
            .engine_tx
            .send(EngineCommand::Crud {
                request_id,
                action,
                kind: EngineRequestKind::SelectTop,
            })
            .is_ok()
        {
            self.schema.mark_auto_fetch_dispatched(table);
            self.status.info(format!(
                "Fetching top {limit} row(s) from {}...",
                table.display_name()
            ));
            self.start_request(
                request_id,
                EngineRequestKind::SelectTop,
                table.display_name(),
                None,
                false,
            );
            true
        } else {
            self.status.warning("Engine unavailable");
            false
        }
    }

    pub(crate) fn preview_sql(sql: &str) -> String {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return "query".to_string();
        }
        let first_line = trimmed.lines().next().unwrap_or("");
        if first_line.chars().count() > 48 {
            format!("{}…", first_line.chars().take(48).collect::<String>())
        } else {
            first_line.to_string()
        }
    }

    pub(crate) fn relation_kind_for_table(
        &self,
        table: &QualifiedRelation,
    ) -> Option<RelationKind> {
        self.catalog
            .tables
            .iter()
            .find(|metadata| metadata.relation() == *table)
            .map(|metadata| metadata.relation_kind)
    }
}
