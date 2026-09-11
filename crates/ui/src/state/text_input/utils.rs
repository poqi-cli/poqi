use std::convert::TryFrom;

use super::INDENT_WIDTH;

pub(crate) fn prev_char_boundary(line: &str, idx: usize) -> Option<usize> {
    if idx == 0 || idx > line.len() {
        return None;
    }
    line[..idx].char_indices().next_back().map(|(pos, _)| pos)
}

pub(crate) fn next_char_boundary(line: &str, idx: usize) -> usize {
    if idx >= line.len() {
        return line.len();
    }
    let mut chars = line[idx..].char_indices();
    if let Some((_, ch)) = chars.next() {
        idx + ch.len_utf8()
    } else {
        line.len()
    }
}

pub(crate) fn normalize_positions(
    a: (usize, usize),
    b: (usize, usize),
) -> ((usize, usize), (usize, usize)) {
    if a.0 < b.0 || (a.0 == b.0 && a.1 <= b.1) {
        (a, b)
    } else {
        (b, a)
    }
}

pub(crate) fn remove_line_indent(line: &mut String) -> usize {
    let available = line.chars().take_while(|ch| *ch == ' ').count();
    let to_remove = available.min(INDENT_WIDTH);
    if to_remove > 0 {
        line.drain(..to_remove);
    }
    to_remove
}

pub(crate) fn apply_deltas(mut pos: (usize, usize), deltas: &[(usize, isize)]) -> (usize, usize) {
    for (row, delta) in deltas {
        if *row != pos.0 {
            continue;
        }
        if *delta > 0 {
            let magnitude = usize::try_from(*delta).expect("positive delta must fit usize");
            pos.1 = pos.1.saturating_add(magnitude);
        } else if *delta < 0 {
            let magnitude = usize::try_from(
                delta
                    .checked_neg()
                    .expect("negative delta must be in range"),
            )
            .expect("negative delta magnitude must fit usize");
            pos.1 = pos.1.saturating_sub(magnitude.min(pos.1));
        }
    }
    pos
}

pub(crate) fn count_chars(value: &str) -> usize {
    value.chars().count()
}

pub(crate) fn take_chars(value: &str, start: usize, len: usize) -> String {
    value.chars().skip(start).take(len).collect()
}
