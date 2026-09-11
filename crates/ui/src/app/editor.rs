//! Editor-centric helpers for `App`.
//!
//! Keeps cursor movement, completion refresh, and overlay plumbing out of `mod.rs`.

use super::{text_nav, App};

impl App {
    pub(super) fn refresh_editor_completions(&mut self) {
        if !self.editor.needs_completion_refresh {
            return;
        }
        let text = self.editor.text();
        let cursor_offset = self.editor.cursor_offset();
        let batch = self.completion_service.suggest(&text, cursor_offset);
        self.editor.set_completions(batch);
    }

    pub(super) fn move_editor_cursor_vertical(&mut self, delta: isize, step: usize, extend: bool) {
        text_nav::move_cursor_vertical(&mut self.editor, delta, step, extend);
        self.refresh_editor_after_cursor_move();
    }

    pub(super) fn move_editor_cursor_horizontal(
        &mut self,
        delta: isize,
        step: usize,
        extend: bool,
    ) {
        text_nav::move_cursor_horizontal(&mut self.editor, delta, step, extend);
        self.refresh_editor_after_cursor_move();
    }

    pub(super) fn refresh_editor_after_cursor_move(&mut self) {
        self.editor.mark_completions_dirty();
        self.refresh_editor_completions();
        self.clamp_editor_scroll_to_cursor();
    }

    pub(super) fn prepare_editor_selection(&mut self, extend: bool) {
        self.editor.prepare_selection(extend);
    }

    pub(super) fn show_editor_overlay(&mut self, sql: &str) {
        if self.editor_overlay.is_none() {
            self.editor_overlay = Some(EditorOverlay {
                original_text: self.editor.text(),
            });
        }
        self.editor.set_text(sql);
        self.editor.move_to_end();
        self.editor.clear_selection();
    }

    pub(super) fn clear_editor_overlay(&mut self) {
        if let Some(snapshot) = self.editor_overlay.take() {
            self.editor.set_text(&snapshot.original_text);
            self.editor.move_to_end();
            self.editor.clear_selection();
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct EditorOverlay {
    pub(super) original_text: String,
}
