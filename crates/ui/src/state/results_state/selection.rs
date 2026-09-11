use super::ResultsState;

impl ResultsState {
    pub(crate) fn move_vertical(&mut self, delta: isize, extend: bool) {
        let (row, col) = self.focus_cell;
        let new_row = adjust_index(row, delta);
        self.set_focus(new_row, col, extend);
    }

    pub(crate) fn move_horizontal(&mut self, delta: isize, extend: bool) {
        let (row, col) = self.focus_cell;
        let new_col = adjust_index(col, delta);
        self.set_focus(row, new_col, extend);
    }

    pub(crate) fn jump_row_start(&mut self) {
        self.set_focus(0, self.focus_cell.1, false);
    }

    pub(crate) fn jump_row_end(&mut self) {
        let (rows, _) = self.dimensions();
        if rows > 0 {
            self.set_focus(rows - 1, self.focus_cell.1, false);
        }
    }

    pub(crate) fn jump_col_start(&mut self) {
        self.set_focus(self.focus_cell.0, 0, false);
    }

    pub(crate) fn jump_col_end(&mut self) {
        let (_, cols) = self.dimensions();
        if cols > 0 {
            self.set_focus(self.focus_cell.0, cols - 1, false);
        }
    }

    pub(crate) fn selection_bounds(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.selection_anchor?;
        let tail = self.selection_tail.unwrap_or(anchor);
        Some((
            (anchor.0.min(tail.0), anchor.1.min(tail.1)),
            (anchor.0.max(tail.0), anchor.1.max(tail.1)),
        ))
    }

    pub(crate) fn set_focus(&mut self, row: usize, col: usize, extend: bool) {
        let (max_rows, max_cols) = self.dimensions();
        if max_rows == 0 || max_cols == 0 {
            return;
        }
        let row = row.min(max_rows - 1);
        let col = col.min(max_cols - 1);
        self.focus_cell = (row, col);
        self.suspend_focus_row_sync = false;
        self.suspend_focus_col_sync = false;
        self.pending_delete_row = None;
        if self
            .edit_session
            .as_ref()
            .is_some_and(|session| (session.row, session.column) != self.focus_cell)
        {
            self.edit_session = None;
        }
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.focus_cell);
            }
            self.selection_tail = Some(self.focus_cell);
        } else {
            self.selection_anchor = Some(self.focus_cell);
            self.selection_tail = Some(self.focus_cell);
        }
    }

    fn dimensions(&self) -> (usize, usize) {
        (self.rows.len(), self.headers.len())
    }
}

fn adjust_index(value: usize, delta: isize) -> usize {
    let step = delta.unsigned_abs();
    if delta.is_negative() {
        value.saturating_sub(step)
    } else {
        value.saturating_add(step)
    }
}
