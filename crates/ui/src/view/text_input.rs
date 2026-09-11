use crate::state::text_input::{count_chars, SelectionRange, TextInputState};

#[derive(Clone, Debug)]
pub(crate) struct WrappedRow {
    pub(crate) text: String,
    pub(crate) source_line: usize,
    pub(crate) start_byte: usize,
    pub(crate) char_len: usize,
    pub(crate) visual_index: usize,
}

impl WrappedRow {
    pub(crate) fn segment(
        text: String,
        source_line: usize,
        start_byte: usize,
        visual_index: usize,
    ) -> Self {
        let char_len = text.chars().count();
        Self {
            text,
            source_line,
            start_byte,
            char_len,
            visual_index,
        }
    }

    pub(crate) fn blank_line(source_line: usize, visual_index: usize) -> Self {
        Self {
            text: String::new(),
            source_line,
            start_byte: 0,
            char_len: 0,
            visual_index,
        }
    }

    pub(crate) fn virtual_row(source_line: usize, visual_index: usize) -> Self {
        Self {
            text: String::new(),
            source_line,
            start_byte: 0,
            char_len: 0,
            visual_index,
        }
    }
}

pub(crate) fn wrapped_content(
    state: &TextInputState,
    width: usize,
) -> (Vec<WrappedRow>, usize, usize) {
    if width == 0 {
        return (vec![WrappedRow::virtual_row(0, 0)], 0, 0);
    }
    if state.lines.is_empty() {
        return (vec![WrappedRow::virtual_row(0, 0)], 0, 0);
    }

    let cursor_line_idx = state.cursor_row.min(state.lines.len().saturating_sub(1));
    let cursor_byte_idx = state
        .lines
        .get(cursor_line_idx)
        .map_or(0, |line| state.cursor_col.min(line.len()));

    let mut rows: Vec<WrappedRow> = Vec::new();
    let mut visual_row = 0usize;
    let mut cursor_visual_row = 0usize;
    let mut cursor_visual_col = 0usize;

    for (line_idx, line) in state.lines.iter().enumerate() {
        if line.is_empty() {
            rows.push(WrappedRow::blank_line(line_idx, visual_row));
            if line_idx == cursor_line_idx {
                cursor_visual_row = visual_row;
                cursor_visual_col = 0;
            }
            visual_row += 1;
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0usize;
        let mut current_start_byte = 0usize;

        for (byte_idx, ch) in line.char_indices() {
            if current_width == width {
                rows.push(WrappedRow::segment(
                    current,
                    line_idx,
                    current_start_byte,
                    visual_row,
                ));
                visual_row += 1;
                current = String::new();
                current_width = 0;
                current_start_byte = byte_idx;
            }
            if line_idx == cursor_line_idx && cursor_byte_idx == byte_idx {
                cursor_visual_row = visual_row;
                cursor_visual_col = current_width;
            }
            current.push(ch);
            current_width += 1;
        }

        if current_width > 0 {
            rows.push(WrappedRow::segment(
                current,
                line_idx,
                current_start_byte,
                visual_row,
            ));
            let last_row_index = visual_row;
            visual_row += 1;

            if line_idx == cursor_line_idx && cursor_byte_idx == line.len() {
                if current_width == width {
                    rows.push(WrappedRow::virtual_row(line_idx, visual_row));
                    cursor_visual_row = visual_row;
                    cursor_visual_col = 0;
                    visual_row += 1;
                } else {
                    cursor_visual_row = last_row_index;
                    cursor_visual_col = current_width;
                }
            }
        }
    }

    if rows.is_empty() {
        rows.push(WrappedRow::virtual_row(0, 0));
    }

    (rows, cursor_visual_row, cursor_visual_col)
}

pub(crate) fn visible_rows(
    rows: &[WrappedRow],
    height: usize,
    scroll_row: usize,
) -> (Vec<WrappedRow>, usize) {
    if height == 0 {
        return (Vec::new(), 0);
    }

    let total_rows = rows.len().max(1);
    let clamped_height = height.max(1);
    let max_start = total_rows.saturating_sub(clamped_height);
    let start = scroll_row.min(max_start);
    let end = (start + clamped_height).min(total_rows);
    let mut view_rows: Vec<WrappedRow> = rows[start..end].to_vec();

    let mut next_visual = view_rows
        .last()
        .map_or(start, |row| row.visual_index.saturating_add(1));
    while view_rows.len() < clamped_height {
        let source_line = rows.last().map_or(0, |row| row.source_line);
        view_rows.push(WrappedRow::virtual_row(source_line, next_visual));
        next_visual = next_visual.saturating_add(1);
    }

    (view_rows, start)
}

pub(crate) fn byte_offset_from(line: &str, start_byte: usize, char_delta: usize) -> usize {
    if start_byte >= line.len() {
        return line.len();
    }
    if char_delta == 0 {
        return start_byte;
    }
    let mut offset = start_byte;
    let mut remaining = char_delta;
    for ch in line[start_byte..].chars() {
        if remaining == 0 {
            break;
        }
        offset += ch.len_utf8();
        remaining -= 1;
        if remaining == 0 {
            break;
        }
    }
    offset
}

pub(crate) fn selection_span_for_row(
    state: &TextInputState,
    row: &WrappedRow,
    selection: &SelectionRange,
) -> Option<(usize, usize)> {
    if row.text.is_empty() {
        return None;
    }
    let line = state.lines.get(row.source_line)?;
    let (start_line, start_col) = selection.start;
    let (end_line, end_col) = selection.end;
    if row.source_line < start_line || row.source_line > end_line {
        return None;
    }
    let line_len = line.len();
    let row_start = row.start_byte.min(line_len);
    let row_end = (row_start + row.text.len()).min(line_len);
    let selection_start = if row.source_line == start_line {
        start_col.min(line_len)
    } else {
        0
    };
    let selection_end = if row.source_line == end_line {
        end_col.min(line_len)
    } else {
        line_len
    };
    let intersect_start = selection_start.max(row_start);
    let intersect_end = selection_end.min(row_end);
    if intersect_start >= intersect_end {
        return None;
    }
    let relative_start = count_chars(&line[row_start..intersect_start]);
    let relative_len = count_chars(&line[intersect_start..intersect_end]);
    Some((relative_start, relative_start + relative_len))
}

pub(crate) fn hit_test(
    state: &TextInputState,
    view_width: usize,
    scroll_row: usize,
    view_row: usize,
    view_col: usize,
) -> Option<(usize, usize)> {
    if view_width == 0 {
        return None;
    }
    let (wrapped_rows, _, _) = wrapped_content(state, view_width);
    if wrapped_rows.is_empty() {
        return Some((0, 0));
    }
    let content_col = view_col.min(view_width.saturating_sub(1));
    let max_row = wrapped_rows.len().saturating_sub(1);
    let absolute_row = scroll_row.saturating_add(view_row).min(max_row);
    let row = wrapped_rows.get(absolute_row)?;
    let line = state.lines.get(row.source_line)?;
    let target_char = content_col.min(row.char_len);
    let byte_offset = byte_offset_from(line, row.start_byte, target_char);
    Some((row.source_line, byte_offset))
}
