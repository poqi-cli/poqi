use super::TextInputState;
use crate::state::text_input::utils::{apply_deltas, normalize_positions};

/// Normalized byte ranges for selected text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionRange {
    pub(crate) start: (usize, usize),
    pub(crate) end: (usize, usize),
}

impl TextInputState {
    pub(crate) fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    #[must_use]
    pub(crate) fn selection_range(&self) -> Option<SelectionRange> {
        let anchor = self.selection_anchor?;
        let head = self.cursor_position();
        if anchor == head {
            return None;
        }
        let (start, end) = normalize_positions(anchor, head);
        Some(SelectionRange { start, end })
    }

    #[must_use]
    pub(crate) fn selected_text(&self) -> Option<String> {
        let range = self.selection_range()?;
        Some(self.collect_range_text(range.start, range.end))
    }

    pub(crate) fn collapse_selection(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            return false;
        };
        self.remove_range(range.start, range.end);
        self.set_cursor(range.start.0, range.start.1);
        self.selection_anchor = None;
        true
    }

    #[must_use]
    pub(crate) fn selection_line_bounds(&self, range: SelectionRange) -> (usize, usize) {
        let start = range.start.0.min(self.lines.len().saturating_sub(1));
        let end = range.end.0.min(self.lines.len().saturating_sub(1));
        if end < start {
            (start, start)
        } else {
            (start, end)
        }
    }

    pub(crate) fn adjust_selection_positions(&mut self, deltas: &[(usize, isize)]) {
        let Some(anchor) = self.selection_anchor else {
            return;
        };
        let head = (self.cursor_row, self.cursor_col);
        let adjusted_anchor = apply_deltas(anchor, deltas);
        let adjusted_head = apply_deltas(head, deltas);
        self.selection_anchor = Some(adjusted_anchor);
        self.set_cursor(adjusted_head.0, adjusted_head.1);
    }

    fn collect_range_text(&self, start: (usize, usize), end: (usize, usize)) -> String {
        if start.0 == end.0 {
            return self.lines.get(start.0).map_or_else(String::new, |line| {
                line[start.1.min(line.len())..end.1.min(line.len())].to_string()
            });
        }
        let mut parts = Vec::new();
        if let Some(line) = self.lines.get(start.0) {
            let slice = &line[start.1.min(line.len())..];
            parts.push(slice.to_string());
        }
        for idx in start.0 + 1..end.0 {
            if let Some(line) = self.lines.get(idx) {
                parts.push(line.clone());
            }
        }
        if let Some(line) = self.lines.get(end.0) {
            let slice = &line[..end.1.min(line.len())];
            parts.push(slice.to_string());
        }
        parts.join("\n")
    }

    fn remove_range(&mut self, start: (usize, usize), end: (usize, usize)) {
        if start.0 == end.0 {
            if let Some(line) = self.lines.get_mut(start.0) {
                let begin = start.1.min(line.len());
                let finish = end.1.min(line.len());
                if begin < finish {
                    line.drain(begin..finish);
                }
            }
            return;
        }
        let tail = self
            .lines
            .get(end.0)
            .map(|line| line[end.1.min(line.len())..].to_string())
            .unwrap_or_default();
        if let Some(line) = self.lines.get_mut(start.0) {
            let truncate_at = start.1.min(line.len());
            line.truncate(truncate_at);
            line.push_str(&tail);
        }
        if start.0 < end.0 && end.0 < self.lines.len() {
            self.lines.drain((start.0 + 1)..=end.0);
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }
}
