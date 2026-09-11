use poqi_catalog::QualifiedRelation;
use poqi_engine::{ResultOrigin, RowIdentity, RunSqlRefresh};

use super::ResultsState;

#[derive(Debug, Clone, Copy)]
pub(crate) enum RefreshStrategy<'a> {
    SelectTop { table: &'a QualifiedRelation },
    RerunSql { sql: &'a str },
    RowRefresh { table: &'a QualifiedRelation },
    None,
}

impl ResultsState {
    pub(crate) fn remove_row(&mut self, row: usize) {
        if row >= self.rows.len() {
            return;
        }
        self.rows.remove(row);
        if row < self.row_identities.len() {
            self.row_identities.remove(row);
        }
        if row < self.null_cells.len() {
            self.null_cells.remove(row);
        }
        if self.rows.is_empty() {
            self.focus_cell = (0, 0);
            self.selection_anchor = None;
            self.selection_tail = None;
            self.set_scroll_row(0);
        } else {
            if self.focus_cell.0 >= self.rows.len() {
                self.focus_cell.0 = self.rows.len().saturating_sub(1);
            }
            let col = self.focus_cell.1.min(self.headers.len().saturating_sub(1));
            self.focus_cell.1 = col;
            self.selection_anchor = Some((self.focus_cell.0, col));
            self.selection_tail = self.selection_anchor;
            let max_scroll = self.rows.len().saturating_sub(1);
            let new_scroll = self.scroll_row.min(max_scroll);
            self.set_scroll_row(new_scroll);
        }
        self.pending_delete_row = None;
        self.edit_session = None;
        if self.rows.is_empty() {
            self.set_scroll_row(0);
        }
    }

    pub(crate) fn apply_cell_update(&mut self, row: usize, column: usize, value: Option<String>) {
        let is_null = value.is_none();
        let display_value = value.as_deref().unwrap_or("NULL");
        let display_width = super::presentation::measured_column_width(display_value);
        let updated = if let Some(cell) = self
            .rows
            .get_mut(row)
            .and_then(|row_values| row_values.get_mut(column))
        {
            *cell = value.unwrap_or_else(|| "NULL".to_string());
            true
        } else {
            false
        };
        if updated {
            self.grow_presentation_width_to(column, display_width);
        }
        if let Some(flag) = self
            .null_cells
            .get_mut(row)
            .and_then(|row_values| row_values.get_mut(column))
        {
            *flag = is_null;
        }
    }

    pub(crate) fn update_row_identity(&mut self, row: usize, identity: RowIdentity) {
        if row < self.row_identities.len() {
            self.row_identities[row] = Some(identity);
        }
    }

    pub(crate) fn arm_pending_delete(&mut self) {
        self.pending_delete_row = Some(self.focus_cell.0);
    }

    pub(crate) fn pending_delete_active(&self) -> bool {
        self.pending_delete_row
            .is_some_and(|row| row == self.focus_cell.0)
    }

    pub(crate) fn clear_pending_delete(&mut self) {
        self.pending_delete_row = None;
    }

    pub(crate) fn refresh_strategy(&self) -> RefreshStrategy<'_> {
        match &self.result_origin {
            ResultOrigin::SelectTop { table, .. } => RefreshStrategy::SelectTop { table },
            ResultOrigin::RunSql {
                sql,
                refresh: RunSqlRefresh::SimpleSingleTable { .. },
            } => RefreshStrategy::RerunSql { sql },
            ResultOrigin::RunSql {
                refresh: RunSqlRefresh::ComplexSingleTable { table },
                ..
            } => RefreshStrategy::RowRefresh { table },
            ResultOrigin::Unknown => RefreshStrategy::None,
        }
    }

    pub(crate) fn hydrate_row(
        &mut self,
        row: usize,
        columns: &[String],
        values: &[String],
        nulls: &[bool],
        row_identity: Option<RowIdentity>,
    ) {
        if row >= self.rows.len() {
            return;
        }
        if let Some(identity) = row_identity {
            self.update_row_identity(row, identity);
        }
        let mut changed_widths = Vec::new();
        if let Some(row_values) = self.rows.get_mut(row) {
            for (idx, source_column) in self.source_columns.iter().enumerate() {
                if let Some(col_idx) = source_column
                    .as_deref()
                    .and_then(|source| columns.iter().position(|name| name == source))
                {
                    if let Some(value) = values.get(col_idx) {
                        if let Some(cell) = row_values.get_mut(idx) {
                            cell.clone_from(value);
                            changed_widths
                                .push((idx, super::presentation::measured_column_width(value)));
                        }
                        if let Some(flag) = self
                            .null_cells
                            .get_mut(row)
                            .and_then(|row_nulls| row_nulls.get_mut(idx))
                        {
                            *flag = nulls.get(col_idx).copied().unwrap_or(false);
                        }
                    }
                }
            }
        }
        for (column, width) in changed_widths {
            self.grow_presentation_width_to(column, width);
        }
    }
}
