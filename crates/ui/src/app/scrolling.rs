use ratatui::layout::Rect;

#[derive(Debug, Clone)]
pub(crate) struct ScrollbarContext {
    pub track: Rect,
    pub viewport_items: usize,
    pub max_scroll: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ScrollDrag {
    pub axis: ScrollbarAxis,
    pub context: ScrollbarContext,
    pub grab_offset: i32,
    pub target: ScrollTarget,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ScrollTarget {
    Results,
    Schema,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ScrollbarAxis {
    Vertical,
    Horizontal,
}
