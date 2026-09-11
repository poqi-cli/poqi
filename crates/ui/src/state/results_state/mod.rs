mod editing;
mod mutation;
mod presentation;
mod scroll;
mod selection;

pub(crate) use editing::{plain_text_modifiers, EditCompletion, EditSession};
pub(crate) use mutation::RefreshStrategy;
pub(crate) use presentation::{bounded_prefix, MAX_DISPLAY_COLUMN_WIDTH};

use poqi_catalog::QualifiedRelation;
use poqi_engine::{ResultMetadata, ResultOrigin, RowIdentity};
use ratatui::widgets::ScrollbarState;

#[derive(Debug, Clone)]
pub(crate) struct ResultsState {
    pub(crate) headers: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) row_identities: Vec<Option<RowIdentity>>,
    pub(crate) source_table: Option<QualifiedRelation>,
    pub(crate) source_columns: Vec<Option<String>>,
    pub(crate) column_types: Vec<String>,
    pub(crate) null_cells: Vec<Vec<bool>>,
    display_headers: Vec<String>,
    presentation_widths: Vec<u16>,
    pub(crate) focus_cell: (usize, usize),
    scroll_row: usize,
    scroll_col: usize,
    scrollbar_state: ScrollbarState,
    horizontal_scrollbar_state: ScrollbarState,
    selection_anchor: Option<(usize, usize)>,
    selection_tail: Option<(usize, usize)>,
    edit_session: Option<EditSession>,
    pending_delete_row: Option<usize>,
    pub(crate) result_origin: ResultOrigin,
    suspend_focus_row_sync: bool,
    suspend_focus_col_sync: bool,
}

impl ResultsState {
    pub(crate) fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
            row_identities: Vec::new(),
            source_table: None,
            source_columns: Vec::new(),
            column_types: Vec::new(),
            null_cells: Vec::new(),
            display_headers: Vec::new(),
            presentation_widths: Vec::new(),
            focus_cell: (0, 0),
            scroll_row: 0,
            scroll_col: 0,
            scrollbar_state: ScrollbarState::new(0),
            selection_anchor: None,
            selection_tail: None,
            edit_session: None,
            pending_delete_row: None,
            result_origin: ResultOrigin::Unknown,
            horizontal_scrollbar_state: ScrollbarState::new(0),
            suspend_focus_row_sync: false,
            suspend_focus_col_sync: false,
        }
    }

    pub(crate) fn set_data(
        &mut self,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        metadata: ResultMetadata,
    ) {
        // Reset selection, focus, and scroll whenever a new result set arrives.
        let (display_headers, presentation_widths) =
            presentation::build_presentation_cache(&headers, &rows);
        self.headers = headers;
        self.rows = rows;
        self.display_headers = display_headers;
        self.presentation_widths = presentation_widths;
        self.source_table = metadata.source_table;
        if metadata.row_identities.len() == self.rows.len() {
            self.row_identities = metadata.row_identities;
        } else {
            self.row_identities = vec![None; self.rows.len()];
        }
        if metadata.source_columns.len() == self.headers.len() {
            self.source_columns = metadata.source_columns;
        } else {
            self.source_columns = vec![None; self.headers.len()];
        }
        if metadata.column_types.len() == self.headers.len() {
            self.column_types = metadata.column_types;
        } else {
            self.column_types = vec![String::new(); self.headers.len()];
        }
        if metadata.null_cells.len() == self.rows.len()
            && metadata
                .null_cells
                .iter()
                .all(|row| row.len() == self.headers.len())
        {
            self.null_cells = metadata.null_cells;
        } else {
            self.null_cells = vec![vec![false; self.headers.len()]; self.rows.len()];
        }
        self.result_origin = metadata.origin;
        self.focus_cell = (0, 0);
        self.set_scroll_row(0);
        self.set_scroll_col(0);
        self.suspend_focus_row_sync = false;
        self.suspend_focus_col_sync = false;
        self.edit_session = None;
        self.pending_delete_row = None;
        if self.rows.is_empty() || self.headers.is_empty() {
            self.selection_anchor = None;
            self.selection_tail = None;
        } else {
            self.selection_anchor = Some((0, 0));
            self.selection_tail = Some((0, 0));
        }
    }

    pub(crate) fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn display_header(&self, column: usize) -> Option<&str> {
        self.display_headers.get(column).map(String::as_str)
    }

    pub(crate) fn presentation_widths(&self) -> &[u16] {
        debug_assert_eq!(self.presentation_widths.len(), self.headers.len());
        &self.presentation_widths
    }

    fn grow_presentation_width_to(&mut self, column: usize, measured_width: u16) {
        if let Some(width) = self.presentation_widths.get_mut(column) {
            *width = (*width).max(measured_width);
        }
    }

    pub(crate) fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    pub(crate) fn scroll_col(&self) -> usize {
        self.scroll_col
    }

    pub(crate) fn set_scroll_row(&mut self, row: usize) {
        self.scroll_row = row;
    }

    pub(crate) fn set_scroll_col(&mut self, col: usize) {
        let max_scroll = self.max_horizontal_scroll();
        self.scroll_col = col.min(max_scroll);
    }
}

#[cfg(test)]
mod tests;
