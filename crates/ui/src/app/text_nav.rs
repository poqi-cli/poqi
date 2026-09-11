use crate::state::{editor_state::EditorState, semantic::SemanticSearchState};
use std::ops::DerefMut;

/// Shared cursor helpers so editor and semantic panels stay in sync.
pub(crate) trait TextNavigator {
    fn prepare_selection(&mut self, extend: bool);
    fn clear_selection(&mut self);
    fn move_up(&mut self);
    fn move_down(&mut self);
    fn move_left(&mut self);
    fn move_right(&mut self);
}

impl TextNavigator for EditorState {
    fn prepare_selection(&mut self, extend: bool) {
        EditorState::deref_mut(self).prepare_selection(extend);
    }

    fn clear_selection(&mut self) {
        EditorState::deref_mut(self).clear_selection();
    }

    fn move_up(&mut self) {
        EditorState::deref_mut(self).move_up();
    }

    fn move_down(&mut self) {
        EditorState::deref_mut(self).move_down();
    }

    fn move_left(&mut self) {
        EditorState::deref_mut(self).move_left();
    }

    fn move_right(&mut self) {
        EditorState::deref_mut(self).move_right();
    }
}

impl TextNavigator for SemanticSearchState {
    fn prepare_selection(&mut self, extend: bool) {
        SemanticSearchState::prepare_selection(self, extend);
    }

    fn clear_selection(&mut self) {
        SemanticSearchState::clear_selection(self);
    }

    fn move_up(&mut self) {
        SemanticSearchState::move_up(self);
    }

    fn move_down(&mut self) {
        SemanticSearchState::move_down(self);
    }

    fn move_left(&mut self) {
        SemanticSearchState::move_left(self);
    }

    fn move_right(&mut self) {
        SemanticSearchState::move_right(self);
    }
}

pub(crate) fn move_cursor_vertical<T: TextNavigator>(
    target: &mut T,
    delta: isize,
    step: usize,
    extend: bool,
) {
    if step == 0 || delta == 0 {
        if !extend {
            target.clear_selection();
        }
        return;
    }
    target.prepare_selection(extend);
    for _ in 0..step {
        if delta < 0 {
            target.move_up();
        } else {
            target.move_down();
        }
    }
}

pub(crate) fn move_cursor_horizontal<T: TextNavigator>(
    target: &mut T,
    delta: isize,
    step: usize,
    extend: bool,
) {
    if step == 0 || delta == 0 {
        if !extend {
            target.clear_selection();
        }
        return;
    }
    target.prepare_selection(extend);
    for _ in 0..step {
        if delta < 0 {
            target.move_left();
        } else {
            target.move_right();
        }
    }
}
