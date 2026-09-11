use poqi_search_completion::CompletionItem;
use smallvec::SmallVec;

const DEFAULT_VIEWPORT_ROWS: usize = 6;

#[derive(Debug, Clone)]
pub(crate) struct CompletionPopup {
    items: SmallVec<[CompletionItem; 16]>,
    selected: Option<usize>,
    scroll_offset: usize,
    viewport_rows: usize,
}

impl CompletionPopup {
    pub(crate) fn new() -> Self {
        Self {
            items: SmallVec::new(),
            selected: None,
            scroll_offset: 0,
            viewport_rows: DEFAULT_VIEWPORT_ROWS,
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        !self.items.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.selected = None;
        self.scroll_offset = 0;
    }

    pub(crate) fn set_items(&mut self, items: SmallVec<[CompletionItem; 16]>) {
        // Drop the popup if we have no suggestions; otherwise reset with the new batch.
        if items.is_empty() {
            self.clear();
            return;
        }
        self.items = items;
        self.selected = None;
        self.scroll_offset = 0;
        self.clamp_scroll_offset();
        self.ensure_selected_visible();
    }

    pub(crate) fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.selected
            .map(|idx| idx.min(self.items.len().saturating_sub(1)))
    }

    pub(crate) fn select_next(&mut self) {
        // Walk the selection downward but never wrap; the caller decides about wrap-around.
        if self.items.is_empty() {
            return;
        }
        if let Some(selected) = self.selected {
            let last = self.items.len().saturating_sub(1);
            if selected < last {
                self.selected = Some(selected + 1);
                self.ensure_selected_visible();
            }
        } else {
            self.selected = Some(0);
            self.ensure_selected_visible();
        }
    }

    pub(crate) fn select_previous(&mut self) {
        // Mirror of `select_next`, clamped at the start of the list.
        if self.items.is_empty() {
            return;
        }
        if let Some(selected) = self.selected {
            if selected > 0 {
                self.selected = Some(selected - 1);
                self.ensure_selected_visible();
            }
        } else {
            self.selected = Some(self.items.len().saturating_sub(1));
            self.ensure_selected_visible();
        }
    }

    pub(crate) fn current(&self) -> Option<&CompletionItem> {
        self.selected_index().map(|idx| &self.items[idx])
    }

    pub(crate) fn set_selected_index(&mut self, index: usize) {
        if self.items.is_empty() {
            return;
        }
        let max_idx = self.items.len().saturating_sub(1);
        self.selected = Some(index.min(max_idx));
        self.ensure_selected_visible();
    }

    pub(crate) fn set_viewport_rows(&mut self, rows: usize) {
        let rows = rows.max(1);
        self.viewport_rows = rows;
        self.clamp_scroll_offset();
        self.ensure_selected_visible();
    }

    pub(crate) fn viewport_rows(&self) -> usize {
        self.viewport_rows.max(1)
    }

    pub(crate) fn visible_range(&self) -> (usize, usize) {
        if self.items.is_empty() {
            return (0, 0);
        }
        let viewport = self.viewport_rows();
        let max_start = self.max_scroll_offset();
        let start = self.scroll_offset.min(max_start);
        let end = (start + viewport).min(self.items.len());
        (start, end)
    }

    pub(crate) fn scroll_viewport(&mut self, delta: isize) {
        if self.items.is_empty() || delta == 0 {
            return;
        }
        let viewport = self.viewport_rows();
        if self.items.len() <= viewport {
            self.scroll_offset = 0;
            self.clamp_selection_to_view();
            return;
        }
        let current = isize::try_from(self.scroll_offset).unwrap_or(0);
        let max_offset = self.max_scroll_offset();
        let clamped = current
            .saturating_add(delta)
            .max(0)
            .min(isize::try_from(max_offset).unwrap_or(isize::MAX));
        self.scroll_offset = usize::try_from(clamped).unwrap_or(0);
        self.clamp_selection_to_view();
    }

    fn ensure_selected_visible(&mut self) {
        if self.items.is_empty() {
            self.scroll_offset = 0;
            return;
        }
        self.clamp_scroll_offset();
        let viewport = self.viewport_rows();
        if let Some(selected) = self.selected {
            if selected < self.scroll_offset {
                self.scroll_offset = selected;
            } else if selected >= self.scroll_offset + viewport {
                self.scroll_offset = selected.saturating_add(1).saturating_sub(viewport);
            }
        }
        self.clamp_scroll_offset();
    }

    fn clamp_selection_to_view(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
            return;
        }
        let viewport = self.viewport_rows();
        let max_idx = self.items.len().saturating_sub(1);
        if let Some(selected) = self.selected {
            let clamped = selected.min(max_idx);
            let adjusted = if clamped < self.scroll_offset {
                self.scroll_offset
            } else if clamped >= self.scroll_offset + viewport {
                (self.scroll_offset + viewport - 1).min(max_idx)
            } else {
                clamped
            };
            self.selected = Some(adjusted);
        }
    }

    fn max_scroll_offset(&self) -> usize {
        let viewport = self.viewport_rows();
        self.items.len().saturating_sub(viewport)
    }

    fn clamp_scroll_offset(&mut self) {
        let max_offset = self.max_scroll_offset();
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }
}

#[cfg(test)]
mod tests;
