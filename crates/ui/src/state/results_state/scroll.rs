use super::ResultsState;

impl ResultsState {
    pub(crate) fn set_manual_scroll_row(&mut self, row: usize) {
        self.suspend_focus_row_sync = true;
        self.set_scroll_row(row);
    }

    pub(crate) fn set_manual_scroll_col(&mut self, col: usize) {
        self.suspend_focus_col_sync = true;
        self.set_scroll_col(col);
    }

    #[cfg(test)]
    pub(crate) fn vertical_scrollbar_state(&self) -> &ratatui::widgets::ScrollbarState {
        &self.scrollbar_state
    }

    #[cfg(test)]
    pub(crate) fn horizontal_scrollbar_state(&self) -> &ratatui::widgets::ScrollbarState {
        &self.horizontal_scrollbar_state
    }

    pub(crate) fn sync_scrollbar_thumb(&mut self, viewport_rows: usize) {
        let viewport = viewport_rows.max(1);
        let total_rows = self.row_count();
        if total_rows == 0 || total_rows <= viewport {
            self.scrollbar_state = self
                .scrollbar_state
                .content_length(0)
                .viewport_content_length(viewport)
                .position(0);
            return;
        }
        let max_top = total_rows.saturating_sub(viewport);
        let clamped_top = self.scroll_row.min(max_top);
        self.scroll_row = clamped_top;
        let scrollable_positions = max_top.saturating_add(1);
        self.scrollbar_state = self
            .scrollbar_state
            .content_length(scrollable_positions)
            .viewport_content_length(viewport)
            .position(clamped_top);
    }

    pub(crate) fn sync_horizontal_scrollbar(&mut self, viewport_columns: usize) {
        let total_columns = self.headers.len();
        if total_columns <= 1 || total_columns <= viewport_columns {
            self.horizontal_scrollbar_state = self
                .horizontal_scrollbar_state
                .content_length(0)
                .position(0);
            return;
        }
        let viewport = viewport_columns.max(1);
        let max_left = self.max_horizontal_scroll();
        let clamped_left = self.scroll_col.min(max_left);
        self.scroll_col = clamped_left;
        let scrollable_positions = max_left.saturating_add(1);
        self.horizontal_scrollbar_state = self
            .horizontal_scrollbar_state
            .content_length(scrollable_positions)
            .viewport_content_length(viewport)
            .position(clamped_left);
    }

    pub(crate) fn ensure_focus_visible(&mut self, visible_rows: usize) {
        // Keep the focused row inside the visible viewport; clamp scroll when data shrinks.
        if self.rows.is_empty() || visible_rows == 0 {
            self.set_scroll_row(0);
            return;
        }
        let max_scroll = self.rows.len().saturating_sub(visible_rows);
        let mut scroll_row = self.scroll_row.min(max_scroll);
        if !self.suspend_focus_row_sync {
            if self.focus_cell.0 < scroll_row {
                scroll_row = self.focus_cell.0;
            } else {
                let max_visible_row = scroll_row.saturating_add(visible_rows.saturating_sub(1));
                if self.focus_cell.0 > max_visible_row {
                    scroll_row = self
                        .focus_cell
                        .0
                        .saturating_add(1)
                        .saturating_sub(visible_rows);
                }
            }
            if scroll_row > max_scroll {
                scroll_row = max_scroll;
            }
        }
        self.set_scroll_row(scroll_row);
    }

    pub(crate) fn ensure_focus_col_visible(
        &mut self,
        max_width: u16,
        separator_width: u16,
        column_widths: &[u16],
    ) {
        // Scroll horizontally until the focused column is fully visible.
        if column_widths.is_empty() || max_width == 0 {
            self.set_scroll_col(0);
            return;
        }

        let max_index = column_widths.len().saturating_sub(1);
        let mut scroll_col = self.scroll_col.min(max_index);
        if self.suspend_focus_col_sync {
            self.set_scroll_col(scroll_col);
            return;
        }
        let focus_col = self.focus_cell.1.min(max_index);
        scroll_col = scroll_col.min(focus_col);

        if !column_is_visible(
            scroll_col,
            focus_col,
            max_width,
            separator_width,
            column_widths,
        ) {
            scroll_col = focus_col;
        }

        while scroll_col > 0 {
            let prev = scroll_col - 1;
            if column_is_visible(prev, focus_col, max_width, separator_width, column_widths) {
                scroll_col = prev;
            } else {
                break;
            }
        }
        self.set_scroll_col(scroll_col);
    }

    pub(crate) fn max_horizontal_scroll(&self) -> usize {
        self.headers.len().saturating_sub(1)
    }
}

fn column_is_visible(
    start_col: usize,
    target_col: usize,
    max_width: u16,
    separator_width: u16,
    column_widths: &[u16],
) -> bool {
    // Simulate the layout to determine if the target column fits within the available width.
    if start_col >= column_widths.len() || target_col >= column_widths.len() || max_width == 0 {
        return false;
    }
    let mut used = 0u16;
    let mut idx = start_col;
    let mut shown = 0usize;

    while idx < column_widths.len() && used < max_width {
        let desired = column_widths[idx].max(1);
        if shown == 0 && desired > max_width {
            return idx == target_col;
        }

        let remaining = max_width.saturating_sub(used);
        if remaining == 0 {
            break;
        }

        if shown == 0 {
            if desired > remaining {
                break;
            }
        } else {
            if remaining <= separator_width {
                break;
            }
            let available = remaining.saturating_sub(separator_width);
            if desired > available {
                break;
            }
            used = used.saturating_add(separator_width);
        }

        used = used.saturating_add(desired);
        if idx == target_col {
            return true;
        }

        shown += 1;
        idx += 1;
    }

    false
}
