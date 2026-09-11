//! Focus and panel-selection helpers for `App`.
//!
//! Centralises all WASD/arrow navigation logic so `app::mod` stays lean.

use crate::input::FocusPanel;

use super::{App, UiLayer};

impl App {
    pub(super) fn enter_window_layer(&mut self) {
        self.layer = UiLayer::PanelSelect;
        self.panel_cursor = self.focus;
        if self.focus.is_top_row() {
            self.panel_select_last_top = self.focus;
        }
        self.status.info(
            "Panel select: WASD/arrows to choose, Shift for fast, F to focus, Esc to go back, Ctrl+C to quit",
        );
    }

    pub(super) fn enter_window_layer_with_cursor(&mut self, panel: FocusPanel) {
        self.enter_window_layer();
        self.panel_cursor = panel;
        if panel.is_top_row() {
            self.panel_select_last_top = panel;
        }
    }

    pub(super) fn focus_selected_panel(&mut self) {
        let panel = self.panel_cursor;
        self.focus_window(panel);
        self.status.info(format!("Focus -> {:?}", self.focus));
    }

    pub(super) fn focus_window(&mut self, panel: FocusPanel) {
        let focus_changed = self.focus != panel;
        if focus_changed {
            self.zoomed_panel = None;
        }
        self.focus = panel;
        self.panel_cursor = panel;
        if panel.is_top_row() {
            self.panel_select_last_top = panel;
        }
        self.layer = UiLayer::PanelFocused;
        if !matches!(panel, FocusPanel::Editor) {
            self.editor.clear_completions();
        }
        if matches!(panel, FocusPanel::Schema) {
            self.auto_fetch_on_table_selection();
        }
        if focus_changed && matches!(panel, FocusPanel::Editor) {
            self.editor.move_to_end();
            self.editor.mark_completions_dirty();
        }
    }

    pub(super) fn move_selection(&mut self, forward: bool) {
        self.move_selection_horizontal(forward);
    }

    pub(super) fn move_selection_horizontal(&mut self, forward: bool) {
        self.move_selection_horizontal_by(forward, 1);
    }

    pub(super) fn move_selection_horizontal_by(&mut self, forward: bool, steps: usize) {
        if steps == 0 {
            return;
        }
        let mut remaining = steps;
        if !forward && self.panel_cursor == FocusPanel::Results {
            self.panel_cursor = FocusPanel::Schema;
            self.panel_select_last_top = FocusPanel::Schema;
            remaining = remaining.saturating_sub(1);
        }
        for _ in 0..remaining {
            let next = self.panel_select_neighbor(forward);
            if next.is_top_row() {
                self.panel_select_last_top = next;
            }
            self.panel_cursor = next;
        }
    }

    pub(super) fn move_selection_vertical(&mut self, downward: bool) {
        self.move_selection_vertical_by(downward, 1);
    }

    pub(super) fn move_selection_vertical_by(&mut self, downward: bool, steps: usize) {
        for _ in 0..steps {
            self.panel_cursor = if downward {
                match self.panel_cursor {
                    FocusPanel::Schema | FocusPanel::Editor | FocusPanel::SemanticSearch => {
                        self.panel_select_last_top = self.panel_cursor;
                        FocusPanel::Results
                    }
                    FocusPanel::Results | FocusPanel::Status => FocusPanel::Status,
                }
            } else {
                match self.panel_cursor {
                    FocusPanel::Status => FocusPanel::Results,
                    FocusPanel::Results => FocusPanel::Editor,
                    other => other,
                }
            };
        }
    }

    fn panel_select_neighbor(&self, forward: bool) -> FocusPanel {
        match (forward, self.panel_cursor) {
            (true, FocusPanel::Schema) | (false, FocusPanel::SemanticSearch) => FocusPanel::Editor,
            (true, FocusPanel::Editor | FocusPanel::SemanticSearch)
            | (false, FocusPanel::Results) => FocusPanel::SemanticSearch,
            (false, FocusPanel::Editor) => FocusPanel::Schema,
            _ => self.panel_cursor,
        }
    }
}
