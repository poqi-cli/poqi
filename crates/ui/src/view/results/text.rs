use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::state::results_state::bounded_prefix;

pub(crate) fn format_cell_value(value: &str, column_width: u16) -> String {
    // Clamp cell contents to the visible width while preserving grapheme boundaries.
    let mut buffer = String::new();
    let width = usize::from(column_width);
    if width == 0 {
        return buffer;
    }

    let (prefix, byte_truncated) = bounded_prefix(value);
    let mut current_width = 0usize;
    let text_width = UnicodeWidthStr::width(prefix);
    if !byte_truncated && text_width <= width {
        buffer.push_str(prefix);
        current_width = text_width;
    } else if width == 1 {
        buffer.push('…');
        current_width = 1;
    } else {
        let target = width.saturating_sub(1);
        for grapheme in prefix.graphemes(true) {
            let g_width = UnicodeWidthStr::width(grapheme);
            if current_width + g_width > target {
                break;
            }
            buffer.push_str(grapheme);
            current_width += g_width;
        }
        buffer.push('…');
        current_width += 1;
    }

    if current_width < width {
        push_spaces(&mut buffer, width - current_width);
    }
    buffer
}

pub(crate) fn displayed_value_width(value: &str, column_width: u16) -> usize {
    // Mirror the format logic to know where to place the cursor within the rendered cell.
    let width = usize::from(column_width);
    if width == 0 {
        return 0;
    }

    let (prefix, byte_truncated) = bounded_prefix(value);
    let text_width = UnicodeWidthStr::width(prefix);
    if !byte_truncated && text_width <= width {
        return text_width;
    }
    if width == 1 {
        return 1;
    }

    let mut used = 0usize;
    let target = width.saturating_sub(1);
    for grapheme in prefix.graphemes(true) {
        let g_width = UnicodeWidthStr::width(grapheme);
        if used + g_width > target {
            break;
        }
        used += g_width;
    }
    used + 1
}

fn push_spaces(buf: &mut String, count: usize) {
    for _ in 0..count {
        buf.push(' ');
    }
}

pub(crate) fn clip_text_to_width(text: &str, max_width: u16) -> String {
    if max_width == 0 {
        return String::new();
    }
    let (prefix, byte_truncated) = bounded_prefix(text);
    let width = usize::from(max_width);
    let text_width = UnicodeWidthStr::width(prefix);
    if !byte_truncated && text_width <= width {
        return prefix.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }

    let mut buffer = String::new();
    let mut used = 0usize;
    let target = width.saturating_sub(1);
    for grapheme in prefix.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if used.saturating_add(grapheme_width) > target {
            break;
        }
        buffer.push_str(grapheme);
        used = used.saturating_add(grapheme_width);
    }
    buffer.push('…');
    buffer
}
