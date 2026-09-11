use super::identifiers::is_identifier_char;

#[derive(Debug, Clone, Default)]
pub(crate) struct TokenWindow {
    pub(crate) current: String,
    pub(crate) prev_char: Option<char>,
    pub(crate) start_offset: usize,
}

pub(crate) fn current_token(buffer: &str, cursor: usize) -> TokenWindow {
    if buffer.is_empty() {
        return TokenWindow::default();
    }

    let cursor = cursor.min(buffer.len());
    let (line_before, _) = buffer.split_at(cursor);
    let mut start = cursor;
    for (idx, ch) in line_before.char_indices().rev() {
        if is_identifier_char(ch) {
            start = idx;
            continue;
        }
        start = idx + ch.len_utf8();
        break;
    }
    let current = line_before[start..].to_string();

    let prev_char = if start > 0 {
        line_before[..start].chars().next_back()
    } else {
        None
    };

    TokenWindow {
        current,
        prev_char,
        start_offset: start,
    }
}
