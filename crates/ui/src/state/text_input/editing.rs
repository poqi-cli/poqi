use std::convert::TryFrom;

use super::{TextInputState, INDENT, INDENT_WIDTH};
use crate::state::text_input::utils::{prev_char_boundary, remove_line_indent};
use crate::state::text_input::SelectionRange;

impl TextInputState {
    /// Replace the entire buffer, resetting scroll state.
    pub(crate) fn set_text(&mut self, text: &str) {
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(ToString::to_string).collect()
        };
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines.last().map_or(0, String::len);
        self.scroll_row = 0;
        self.selection_anchor = None;
    }

    /// Insert a single character at the cursor, replacing the selection first.
    pub(crate) fn insert_char(&mut self, ch: char) {
        self.collapse_selection();
        self.ensure_line_index(self.cursor_row);
        let insert_at = {
            let line = &self.lines[self.cursor_row];
            self.cursor_col.min(line.len())
        };
        let line = &mut self.lines[self.cursor_row];
        line.insert(insert_at, ch);
        self.cursor_col = insert_at + ch.len_utf8();
    }

    /// Insert arbitrary text (with CR/LF normalization) at the cursor.
    pub(crate) fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.collapse_selection();
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        self.ensure_line_index(self.cursor_row);
        let insert_at = self.cursor_col.min(self.lines[self.cursor_row].len());
        let tail = {
            let line = &mut self.lines[self.cursor_row];
            line.split_off(insert_at)
        };
        let mut parts = normalized.split('\n');
        if let Some(first) = parts.next() {
            self.lines[self.cursor_row].push_str(first);
            self.cursor_col = self.lines[self.cursor_row].len();
        }
        for part in parts {
            self.cursor_row += 1;
            self.lines.insert(self.cursor_row, part.to_string());
            self.cursor_col = self.lines[self.cursor_row].len();
        }
        let insertion_end = self.lines[self.cursor_row].len();
        self.lines[self.cursor_row].push_str(&tail);
        self.cursor_col = insertion_end;
    }

    /// Insert a tab stop, indenting selections or inserting spaces at the caret.
    pub(crate) fn insert_tab(&mut self) {
        if let Some(range) = self.selection_range() {
            self.indent_selection(range);
            return;
        }
        self.insert_text(INDENT);
    }

    /// Remove leading indentation from the current selection or line.
    pub(crate) fn outdent(&mut self) {
        if let Some(range) = self.selection_range() {
            self.outdent_selection(range);
            return;
        }
        self.outdent_current_line();
    }

    /// Split the current line at the cursor, inserting a fresh line below.
    pub(crate) fn newline(&mut self) {
        self.collapse_selection();
        if self.cursor_row >= self.lines.len() {
            self.ensure_line_index(self.cursor_row);
            self.cursor_col = 0;
            return;
        }
        let rest = {
            let current = &mut self.lines[self.cursor_row];
            let split_at = self.cursor_col.min(current.len());
            current.split_off(split_at)
        };
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_row, rest);
    }

    /// Delete the previous character (or merge with the line above).
    pub(crate) fn backspace(&mut self) {
        if self.collapse_selection() {
            return;
        }
        if self.cursor_row >= self.lines.len() {
            return;
        }
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let prev = prev_char_boundary(line, self.cursor_col).unwrap_or(0);
            line.drain(prev..self.cursor_col.min(line.len()));
            self.cursor_col = prev;
        } else if self.cursor_row > 0 {
            let removed = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&removed);
        }
    }

    fn indent_selection(&mut self, range: SelectionRange) {
        let (start_row, end_row) = self.selection_line_bounds(range);
        let mut deltas = Vec::new();
        let indent_delta =
            isize::try_from(INDENT_WIDTH).expect("indent width must fit within isize");
        for row in start_row..=end_row {
            let Some(line) = self.lines.get_mut(row) else {
                continue;
            };
            line.insert_str(0, INDENT);
            deltas.push((row, indent_delta));
        }
        if !deltas.is_empty() {
            self.adjust_selection_positions(&deltas);
        }
    }

    fn outdent_selection(&mut self, range: SelectionRange) {
        let (start_row, end_row) = self.selection_line_bounds(range);
        let mut deltas = Vec::new();
        for row in start_row..=end_row {
            let Some(line) = self.lines.get_mut(row) else {
                continue;
            };
            let removed = remove_line_indent(line);
            if removed > 0 {
                let removed_delta =
                    isize::try_from(removed).expect("indent removal must fit within isize");
                deltas.push((row, -removed_delta));
            }
        }
        if !deltas.is_empty() {
            self.adjust_selection_positions(&deltas);
        }
    }

    fn outdent_current_line(&mut self) {
        self.ensure_line_index(self.cursor_row);
        let Some(line) = self.lines.get_mut(self.cursor_row) else {
            return;
        };
        let removed = remove_line_indent(line);
        if removed == 0 {
            return;
        }
        let to_subtract = removed.min(self.cursor_col);
        self.cursor_col -= to_subtract;
    }
}
