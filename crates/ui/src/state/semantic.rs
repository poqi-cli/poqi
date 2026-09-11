const MAX_QUERY_LEN: usize = 512;
const HINTS: [&str; 3] = [
    "Type natural language to jump to related tables.",
    "Press Enter while focused here to run semantic search.",
    "Use semantic search to filter SELECT * results by meaning.",
];

#[derive(Debug, Clone)]
pub enum SemanticStatus {
    Disabled { reason: String },
    Idle,
    Loading { label: String },
    Ready { summary: String },
    Error { message: String },
}

use super::text_input::{count_chars, take_chars, TextInputState, INDENT};

#[derive(Debug, Clone)]
pub struct SemanticSearchState {
    input: TextInputState,
    last_hint_index: usize,
    status: SemanticStatus,
}

impl Default for SemanticSearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticSearchState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: TextInputState::new(),
            last_hint_index: 0,
            status: SemanticStatus::Idle,
        }
    }

    #[must_use]
    pub fn query(&self) -> String {
        self.input.text()
    }

    #[must_use]
    pub fn buffer(&self) -> &TextInputState {
        &self.input
    }

    #[must_use]
    pub fn status(&self) -> &SemanticStatus {
        &self.status
    }

    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        self.input.selected_text()
    }

    pub fn set_disabled(&mut self, reason: impl Into<String>) {
        self.status = SemanticStatus::Disabled {
            reason: reason.into(),
        };
    }

    pub fn set_loading(&mut self, label: impl Into<String>) {
        self.status = SemanticStatus::Loading {
            label: label.into(),
        };
    }

    pub fn set_ready(&mut self, summary: impl Into<String>) {
        self.status = SemanticStatus::Ready {
            summary: summary.into(),
        };
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = SemanticStatus::Error {
            message: message.into(),
        };
    }

    pub fn set_idle(&mut self) {
        self.status = SemanticStatus::Idle;
    }

    #[must_use]
    pub fn is_disabled(&self) -> bool {
        matches!(self.status, SemanticStatus::Disabled { .. })
    }

    /// Rotate through helper hints to keep the panel informative.
    #[must_use]
    pub fn next_hint(&mut self) -> &str {
        self.last_hint_index = (self.last_hint_index + 1) % HINTS.len();
        HINTS[self.last_hint_index]
    }

    #[must_use]
    pub fn peek_hint(&self) -> &str {
        HINTS[self.last_hint_index % HINTS.len()]
    }

    /// Insert text while respecting the semantic query length limit.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let available = self.available_capacity_for_insert();
        if available == 0 {
            return;
        }
        let truncated = take_chars(&normalized, 0, available);
        if truncated.is_empty() {
            return;
        }
        self.input.insert_text(&truncated);
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.available_capacity_for_insert() == 0 {
            return;
        }
        self.input.insert_char(ch);
    }

    pub fn insert_tab(&mut self) {
        if self.input.selection_range().is_some() {
            self.input.insert_tab();
            return;
        }
        let indent_len = count_chars(INDENT);
        if self.available_capacity() < indent_len {
            return;
        }
        self.input.insert_tab();
    }

    pub fn outdent(&mut self) {
        self.input.outdent();
    }

    pub fn backspace(&mut self) {
        self.input.backspace();
    }

    pub fn prepare_selection(&mut self, extend: bool) {
        self.input.prepare_selection(extend);
    }

    pub fn set_selection_anchor_if_absent(&mut self, anchor: (usize, usize)) {
        if self.input.selection_anchor.is_none() {
            let clamped = self.input.clamp_position(anchor);
            self.input.selection_anchor = Some(clamped);
        }
    }

    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.input.set_cursor(row, col);
    }

    pub fn move_up(&mut self) {
        self.input.move_up();
    }

    pub fn move_down(&mut self) {
        self.input.move_down();
    }

    pub fn move_left(&mut self) {
        self.input.move_left();
    }

    pub fn move_right(&mut self) {
        self.input.move_right();
    }

    pub fn clear_selection(&mut self) {
        self.input.clear_selection();
    }

    pub fn ensure_cursor_visible(
        &mut self,
        cursor_visual_row: usize,
        view_height: usize,
        total_rows: usize,
    ) {
        self.input
            .ensure_cursor_visible(cursor_visual_row, view_height, total_rows);
    }

    pub fn scroll_by(&mut self, delta: isize, total_rows: usize, view_height: usize) {
        self.input.scroll_by(delta, total_rows, view_height);
    }

    fn available_capacity(&self) -> usize {
        MAX_QUERY_LEN.saturating_sub(self.input.char_count())
    }

    fn available_capacity_for_insert(&self) -> usize {
        let selection_chars = self.selection_char_count();
        self.available_capacity().saturating_add(selection_chars)
    }

    fn selection_char_count(&self) -> usize {
        self.input
            .selected_text()
            .map_or(0, |text| count_chars(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticSearchState;

    #[test]
    fn insert_char_respects_limit() {
        let mut state = SemanticSearchState::new();
        for _ in 0..512 {
            state.insert_char('a');
        }
        assert_eq!(state.query().chars().count(), 512);
        state.insert_char('b');
        assert_eq!(state.query().chars().count(), 512);
        assert!(state.query().ends_with('a'));
    }

    #[test]
    fn insert_text_truncates_at_limit() {
        let mut state = SemanticSearchState::new();
        state.insert_text("abc");
        assert_eq!(state.query(), "abc");
        state.insert_text(&"d".repeat(600));
        assert_eq!(state.query().chars().count(), 512);
        assert!(state.query().starts_with("abc"));
    }

    #[test]
    fn insert_text_overwrites_selection_at_limit() {
        let mut state = SemanticSearchState::new();
        state.insert_text(&"a".repeat(512));
        let end = state.input.lines[0].len();
        state.input.selection_anchor = Some((0, 0));
        state.set_cursor(0, end);
        state.insert_text("b");
        assert_eq!(state.query(), "b");
    }

    #[test]
    fn insert_char_overwrites_selection_at_limit() {
        let mut state = SemanticSearchState::new();
        state.insert_text(&"a".repeat(512));
        let end = state.input.lines[0].len();
        state.input.selection_anchor = Some((0, 0));
        state.set_cursor(0, end);
        state.insert_char('b');
        assert_eq!(state.query(), "b");
    }
}
