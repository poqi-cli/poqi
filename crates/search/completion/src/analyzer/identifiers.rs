pub(crate) fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

pub(crate) fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

pub(crate) fn trailing_literal_punctuation(ch: char) -> bool {
    matches!(ch, ':' | ',' | '-' | '+' | '*' | '/' | '%')
}

pub(crate) fn extract_identifier_before_dot(buffer: &str, cursor: usize) -> Option<String> {
    let cursor = cursor.min(buffer.len());
    let before_cursor = buffer.get(..cursor)?;
    let mut token_start = cursor;
    for (idx, ch) in before_cursor.char_indices().rev() {
        if is_identifier_char(ch) {
            token_start = idx;
        } else {
            break;
        }
    }
    let dot = token_start.checked_sub(1)?;
    if buffer.as_bytes().get(dot) != Some(&b'.') {
        return None;
    }
    let line_start = buffer[..dot].rfind('\n').map_or(0, |idx| idx + 1);
    identifier_ending_at(buffer, line_start, dot).map(ToString::to_string)
}

fn identifier_ending_at(buffer: &str, start: usize, end: usize) -> Option<&str> {
    let mut idx = start;
    let mut last_identifier = None;
    while idx < end {
        let ch = buffer[idx..end].chars().next()?;
        if ch == '"' {
            let identifier_start = idx;
            idx += 1;
            while idx < end {
                let current = buffer[idx..end].chars().next()?;
                idx += current.len_utf8();
                if current == '"' {
                    if buffer[idx..end].starts_with('"') {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            last_identifier = Some((identifier_start, idx));
            continue;
        }
        if is_identifier_char(ch) {
            let identifier_start = idx;
            idx += ch.len_utf8();
            while idx < end {
                let current = buffer[idx..end].chars().next()?;
                if !is_identifier_char(current) {
                    break;
                }
                idx += current.len_utf8();
            }
            last_identifier = Some((identifier_start, idx));
            continue;
        }
        idx += ch.len_utf8();
        last_identifier = None;
    }
    last_identifier
        .filter(|(_, identifier_end)| *identifier_end == end)
        .map(|(identifier_start, identifier_end)| &buffer[identifier_start..identifier_end])
}
