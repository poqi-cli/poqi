use crossterm::event::{KeyCode, KeyEvent, KeyEventState, KeyModifiers};

/// Characters that should auto-commit inline completion suggestions.
pub(super) fn is_completion_commit_char(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | ',' | ')' | '(' | ';')
}

pub(super) fn is_ctrl_char(event: &KeyEvent, expected: char) -> bool {
    event.modifiers.contains(KeyModifiers::CONTROL) && char_matches(event, expected)
}

pub(super) fn is_shift_char(event: &KeyEvent, expected: char) -> bool {
    is_shift_held(event) && char_matches(event, expected)
}

pub(super) fn should_run_enter(event: &KeyEvent) -> bool {
    event
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT)
}

fn char_matches(event: &KeyEvent, expected: char) -> bool {
    matches!(event.code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&expected))
}

pub(super) fn is_shift_held(event: &KeyEvent) -> bool {
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        return true;
    }
    match event.code {
        KeyCode::Char(ch) if ch.is_ascii_alphabetic() => {
            let caps_on = event.state.contains(KeyEventState::CAPS_LOCK);
            ch.is_ascii_uppercase() ^ caps_on
        }
        _ => false,
    }
}
