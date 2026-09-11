use unicode_width::UnicodeWidthStr;

use crate::{
    app::App,
    state::results_state::{ResultsState, MAX_DISPLAY_COLUMN_WIDTH},
};

#[derive(Clone)]
pub(super) struct VisibleColumn {
    pub(super) index: usize,
    pub(super) width: u16,
}

pub(super) struct ColumnLayout {
    pub(super) columns: Vec<VisibleColumn>,
    pub(super) offsets: Vec<u16>,
    pub(super) separator_width: u16,
    pub(super) occupied_width: u16,
    pub(super) fully_visible: bool,
}

impl ColumnLayout {
    pub(super) fn empty(separator_width: u16) -> Self {
        Self {
            columns: Vec::new(),
            offsets: Vec::new(),
            separator_width,
            occupied_width: 0,
            fully_visible: true,
        }
    }
}

impl App {
    pub(super) fn column_layout(
        &self,
        max_width: u16,
        separator_width: u16,
        column_widths: &[u16],
    ) -> ColumnLayout {
        // Decide which columns fit based on the current scroll offset and terminal width.
        if column_widths.is_empty() || max_width == 0 {
            return ColumnLayout::empty(separator_width);
        }

        let start_col = self
            .results
            .scroll_col()
            .min(column_widths.len().saturating_sub(1));
        let mut columns = Vec::new();
        let mut used = 0u16;
        let mut idx = start_col;
        let mut fully_visible = true;
        while idx < column_widths.len() && used < max_width {
            let desired = column_widths[idx].max(1);
            let remaining = max_width.saturating_sub(used);
            if remaining == 0 {
                break;
            }

            let mut width = desired.min(remaining);
            let mut truncated = false;
            if columns.is_empty() {
                if desired > remaining {
                    truncated = true;
                }
            } else {
                if remaining <= separator_width {
                    fully_visible = false;
                    break;
                }
                used = used.saturating_add(separator_width);
                let available = max_width.saturating_sub(used);
                if available == 0 {
                    fully_visible = false;
                    break;
                }
                if desired > available {
                    width = available;
                    truncated = true;
                } else {
                    width = desired;
                }
            }

            if width == 0 {
                break;
            }

            columns.push(VisibleColumn { index: idx, width });
            used = used.saturating_add(width);
            idx += 1;

            if truncated {
                fully_visible = false;
                break;
            }
        }

        let offsets = column_offsets(&columns, separator_width);
        let occupied_width = offsets
            .last()
            .zip(columns.last())
            .map_or(0, |(offset, col)| offset.saturating_add(col.width));

        let all_columns_included = idx >= column_widths.len();
        ColumnLayout {
            columns,
            offsets,
            separator_width,
            occupied_width,
            fully_visible: fully_visible && all_columns_included,
        }
    }

    pub(super) fn measure_column_widths(&self) -> Vec<u16> {
        measure_column_widths(&self.results, self.ui_settings.min_column_width())
    }
}

pub(crate) fn measure_column_widths(state: &ResultsState, min_column_width: u16) -> Vec<u16> {
    let minimum = min_column_width.clamp(1, MAX_DISPLAY_COLUMN_WIDTH);
    state
        .presentation_widths()
        .iter()
        .map(|width| (*width).max(minimum).min(MAX_DISPLAY_COLUMN_WIDTH))
        .collect()
}

pub(super) fn column_offsets(columns: &[VisibleColumn], separator_width: u16) -> Vec<u16> {
    // Pre-compute the x-offsets for each visible column, accounting for separators.
    let mut offsets = Vec::with_capacity(columns.len());
    let mut current = 0u16;
    for (idx, column) in columns.iter().enumerate() {
        offsets.push(current);
        current = current.saturating_add(column.width);
        if idx + 1 != columns.len() {
            current = current.saturating_add(separator_width);
        }
    }
    offsets
}

pub(crate) fn estimate_horizontal_viewport_columns(
    max_width: u16,
    separator_width: u16,
    column_widths: &[u16],
) -> usize {
    if max_width == 0 || column_widths.is_empty() {
        return 0;
    }
    let mut used = 0u16;
    let mut count = 0usize;

    for width in column_widths {
        let desired = (*width).max(1);
        if count == 0 && desired > max_width {
            return 1;
        }
        let remaining = max_width.saturating_sub(used);
        if remaining == 0 {
            break;
        }
        if count == 0 {
            if desired > remaining {
                break;
            }
        } else {
            if remaining <= separator_width {
                break;
            }
            let available = remaining.saturating_sub(separator_width);
            if desired > available {
                break;
            }
            used = used.saturating_add(separator_width);
        }
        used = used.saturating_add(desired);
        count += 1;
    }

    count.max(1)
}

pub(crate) fn column_separator_width(separator: &str) -> u16 {
    // Respect the actual width of the unicode separator while keeping a sane fallback.
    let width = UnicodeWidthStr::width(separator)
        .max(1)
        .min(u16::MAX as usize);
    u16::try_from(width).unwrap_or(1)
}
