use ratatui::layout::Rect;

use crate::app::{ResultsHitColumn, ResultsHitbox};

use super::layout::ColumnLayout;

pub(super) fn build_results_hitbox(
    inner_area: Rect,
    header_height: u16,
    start_row: usize,
    layout: &ColumnLayout,
) -> ResultsHitbox {
    let columns = layout
        .columns
        .iter()
        .enumerate()
        .map(|(idx, column)| ResultsHitColumn {
            index: column.index,
            start: layout.offsets.get(idx).copied().unwrap_or_default(),
            width: column.width,
        })
        .collect();

    ResultsHitbox {
        inner_area,
        header_height,
        start_row,
        separator_width: layout.separator_width,
        columns,
    }
}
