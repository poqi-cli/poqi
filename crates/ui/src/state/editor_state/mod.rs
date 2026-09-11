mod completion;

use super::completion_popup_state::CompletionPopup;
use super::text_input::TextInputState;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub(crate) struct EditorState {
    buffer: TextInputState,
    completion_popup: CompletionPopup,
    pub(crate) needs_completion_refresh: bool,
}

impl EditorState {
    pub(crate) fn new() -> Self {
        Self {
            buffer: TextInputState::with_placeholder("-- Type SQL here and press Alt+Enter to run"),
            completion_popup: CompletionPopup::new(),
            needs_completion_refresh: true,
        }
    }

    pub(crate) fn text(&self) -> String {
        self.buffer.text()
    }

    pub(crate) fn set_text(&mut self, text: &str) {
        self.buffer.set_text(text);
        self.needs_completion_refresh = true;
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.buffer.insert_char(ch);
        self.needs_completion_refresh = true;
    }

    pub(crate) fn insert_text(&mut self, text: &str) {
        self.buffer.insert_text(text);
        self.needs_completion_refresh = true;
    }

    pub(crate) fn insert_tab(&mut self) {
        self.buffer.insert_tab();
        self.needs_completion_refresh = true;
    }

    pub(crate) fn outdent(&mut self) {
        self.buffer.outdent();
        self.needs_completion_refresh = true;
    }

    pub(crate) fn newline(&mut self) {
        self.buffer.newline();
        self.needs_completion_refresh = true;
    }

    pub(crate) fn backspace(&mut self) {
        self.buffer.backspace();
        self.needs_completion_refresh = true;
    }
}

#[cfg(test)]
mod tests;

impl Deref for EditorState {
    type Target = TextInputState;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for EditorState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}
