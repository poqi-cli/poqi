mod cursor;
mod editing;
mod selection;
mod utils;

pub(crate) use selection::SelectionRange;
pub(crate) use utils::{count_chars, take_chars};

#[derive(Debug, Clone)]
pub(crate) struct TextInputState {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor_row: usize,
    pub(crate) cursor_col: usize,
    pub(crate) scroll_row: usize,
    pub(crate) selection_anchor: Option<(usize, usize)>,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputState {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            selection_anchor: None,
        }
    }

    #[must_use]
    pub(crate) fn with_placeholder(text: &str) -> Self {
        let mut state = Self::new();
        state.set_text(text);
        state
    }

    #[must_use]
    pub(crate) fn text(&self) -> String {
        self.lines.join("\n")
    }

    #[must_use]
    pub(crate) fn char_count(&self) -> usize {
        self.lines
            .iter()
            .map(|line| line.chars().count())
            .sum::<usize>()
            .saturating_add(self.lines.len().saturating_sub(1))
    }

    pub(crate) fn prepare_selection(&mut self, extend: bool) {
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_position());
            }
        } else {
            self.clear_selection();
        }
    }
}

pub(crate) const INDENT: &str = "    ";
pub(crate) const INDENT_WIDTH: usize = INDENT.len();
