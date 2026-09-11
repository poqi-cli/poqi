use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let horizontal_margin = 100u16.saturating_sub(percent_x);
    let vertical_margin = 100u16.saturating_sub(percent_y);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(vertical_margin / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(vertical_margin - vertical_margin / 2),
        ])
        .split(area);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(horizontal_margin / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(horizontal_margin - horizontal_margin / 2),
        ])
        .split(vertical_chunks[1]);

    horizontal_chunks[1]
}
