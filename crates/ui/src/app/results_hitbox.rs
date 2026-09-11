use ratatui::layout::Rect;

#[derive(Debug, Clone)]
pub(crate) struct ResultsHitbox {
    pub(crate) inner_area: Rect,
    pub(crate) header_height: u16,
    pub(crate) start_row: usize,
    pub(crate) separator_width: u16,
    pub(crate) columns: Vec<ResultsHitColumn>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResultsHitColumn {
    pub(crate) index: usize,
    pub(crate) start: u16,
    pub(crate) width: u16,
}

impl ResultsHitbox {
    pub(crate) fn hit_test(
        &self,
        column: u16,
        row: u16,
        total_rows: usize,
    ) -> Option<(usize, usize)> {
        if self.inner_area.width == 0 || self.inner_area.height == 0 {
            return None;
        }
        let body_start = self
            .inner_area
            .y
            .saturating_add(self.header_height.min(self.inner_area.height));
        let body_end = self.inner_area.y.saturating_add(self.inner_area.height);
        if row < body_start || row >= body_end {
            return None;
        }
        let inner_start_x = self.inner_area.x;
        let inner_end_x = inner_start_x.saturating_add(self.inner_area.width);
        if column < inner_start_x || column >= inner_end_x {
            return None;
        }
        let visible_row = usize::from(row.saturating_sub(body_start));
        let target_row = self.start_row.saturating_add(visible_row);
        if target_row >= total_rows {
            return None;
        }
        let relative_x = column.saturating_sub(inner_start_x);
        let target_col = self.column_at_offset(relative_x)?;
        Some((target_row, target_col))
    }

    fn column_at_offset(&self, offset: u16) -> Option<usize> {
        for column in &self.columns {
            let start = column.start;
            let end = start.saturating_add(column.width);
            if offset >= start && offset < end {
                return Some(column.index);
            }
            if offset < start {
                break;
            }
            if self.separator_width > 0 {
                let sep_start = end;
                let sep_end = sep_start.saturating_add(self.separator_width);
                if offset >= sep_start && offset < sep_end {
                    return Some(column.index);
                }
            }
        }
        None
    }
}
