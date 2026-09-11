use ratatui::layout::Rect;

#[must_use]
pub fn point_in_rect(column: u16, row: u16, rect: &Rect) -> bool {
    column >= rect.x && column < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}
