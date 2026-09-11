use std::convert::TryFrom;

use super::TextInputState;
use crate::state::text_input::utils::{next_char_boundary, prev_char_boundary};

impl TextInputState {
    /// Translate the cursor into an absolute byte offset within the buffer.
    #[must_use]
    pub(crate) fn cursor_offset(&self) -> usize {
        let mut offset = 0usize;
        for (row_idx, line) in self.lines.iter().enumerate() {
            if row_idx == self.cursor_row {
                let col = self.cursor_col.min(line.len());
                offset += col;
                break;
            }
            offset += line.len() + 1;
        }
        offset
    }

    /// Return the cursor position clamped to the current buffer.
    #[must_use]
    pub(crate) fn cursor_position(&self) -> (usize, usize) {
        self.clamp_position((self.cursor_row, self.cursor_col))
    }

    pub(crate) fn set_cursor(&mut self, row: usize, col: usize) {
        self.ensure_line_index(0);
        let (row, col) = self.clamp_position((row, col));
        self.cursor_row = row;
        self.cursor_col = col;
    }

    pub(crate) fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let line_len = self.lines[self.cursor_row].len();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let line_len = self.lines[self.cursor_row].len();
            if self.cursor_col > line_len {
                self.cursor_col = line_len;
            }
        }
    }

    pub(crate) fn move_left(&mut self) {
        if self.cursor_col > 0 {
            if let Some(prev) = prev_char_boundary(&self.lines[self.cursor_row], self.cursor_col) {
                self.cursor_col = prev;
            } else {
                self.cursor_col = 0;
            }
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
    }

    pub(crate) fn move_right(&mut self) {
        if self.cursor_row < self.lines.len() {
            let line_len = self.lines[self.cursor_row].len();
            if self.cursor_col < line_len {
                let next = next_char_boundary(&self.lines[self.cursor_row], self.cursor_col);
                self.cursor_col = next;
            } else if self.cursor_row + 1 < self.lines.len() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
        }
    }

    pub(crate) fn move_to_end(&mut self) {
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines.last().map_or(0, String::len);
    }

    pub(crate) fn scroll_by(&mut self, delta: isize, total_rows: usize, view_height: usize) {
        if total_rows == 0 || view_height == 0 {
            self.scroll_row = 0;
            return;
        }
        let current = isize::try_from(self.scroll_row).unwrap_or(0);
        let next = current.saturating_add(delta);
        let max_start = total_rows.saturating_sub(view_height);
        let clamped = next
            .max(0)
            .min(isize::try_from(max_start).unwrap_or(isize::MAX));
        self.scroll_row = usize::try_from(clamped).unwrap_or(0);
    }

    pub(crate) fn ensure_cursor_visible(
        &mut self,
        cursor_visual_row: usize,
        view_height: usize,
        total_rows: usize,
    ) {
        if view_height == 0 || total_rows == 0 {
            self.scroll_row = 0;
            return;
        }
        let max_start = total_rows.saturating_sub(view_height);
        if cursor_visual_row < self.scroll_row {
            self.scroll_row = cursor_visual_row;
        } else if cursor_visual_row >= self.scroll_row + view_height {
            let diff = cursor_visual_row + 1 - view_height;
            self.scroll_row = diff;
        }
        if self.scroll_row > max_start {
            self.scroll_row = max_start;
        }
    }

    #[must_use]
    pub(crate) fn clamp_position(&self, pos: (usize, usize)) -> (usize, usize) {
        let row = pos.0.min(self.lines.len().saturating_sub(1));
        let col = self.lines.get(row).map_or(0, |line| pos.1.min(line.len()));
        (row, col)
    }

    pub(crate) fn ensure_line_index(&mut self, row: usize) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        while row >= self.lines.len() {
            self.lines.push(String::new());
        }
    }

    pub(crate) fn ensure_line_count(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        let target = count.max(1);
        while self.lines.len() < target {
            self.lines.push(String::new());
        }
    }
}
