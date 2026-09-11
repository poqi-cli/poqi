use crossterm::event::MouseEvent;

use crate::{input::FocusPanel, view::text_input::hit_test as text_hit_test};

use super::super::{App, ScrollDrag, ScrollTarget, ScrollbarAxis, ScrollbarContext};

impl App {
    pub(super) fn handle_mouse_drag(&mut self, mouse: MouseEvent) {
        if let Some(drag) = self.scroll_drag.clone() {
            self.apply_scroll_drag_at(&drag, mouse.column, mouse.row, true);
            return;
        }
        let panel = self
            .panel_under_pointer(mouse)
            .or_else(|| self.editor_mouse_drag_anchor.map(|_| FocusPanel::Editor))
            .or_else(|| {
                self.semantic_mouse_drag_anchor
                    .map(|_| FocusPanel::SemanticSearch)
            });
        match panel {
            Some(FocusPanel::Editor) => self.handle_editor_drag(mouse),
            Some(FocusPanel::SemanticSearch) => self.handle_semantic_drag(mouse),
            _ => {}
        }
    }

    pub(super) fn clear_editor_drag_anchor(&mut self) {
        self.editor_mouse_drag_anchor = None;
    }

    pub(super) fn clear_semantic_drag_anchor(&mut self) {
        self.semantic_mouse_drag_anchor = None;
    }

    fn handle_editor_drag(&mut self, mouse: MouseEvent) {
        let Some((anchor_row, anchor_col)) = self.editor_mouse_drag_anchor else {
            return;
        };
        let Some(text_area) = self.editor_text_area else {
            return;
        };
        if text_area.width == 0 || text_area.height == 0 {
            return;
        }
        let view_width = usize::from(text_area.width.max(1));
        let view_row = clamp_coordinate(mouse.row, text_area.y, text_area.height);
        let view_col = clamp_coordinate(mouse.column, text_area.x, text_area.width);
        if let Some((line_idx, byte_offset)) =
            self.editor_hit_test(view_width, usize::from(view_row), usize::from(view_col))
        {
            let (clamped_row, clamped_col) = self.clamp_editor_hit(line_idx, byte_offset);
            if self.editor.selection_anchor.is_none() {
                self.editor.selection_anchor = Some((anchor_row, anchor_col));
            }
            self.prepare_editor_selection(true);
            self.editor.set_cursor(clamped_row, clamped_col);
            self.refresh_editor_after_cursor_move();
        }
    }

    fn handle_semantic_drag(&mut self, mouse: MouseEvent) {
        let Some((anchor_row, anchor_col)) = self.semantic_mouse_drag_anchor else {
            return;
        };
        let Some(text_area) = self.semantic_text_area else {
            return;
        };
        if text_area.width == 0 || text_area.height == 0 {
            return;
        }
        let view_width = usize::from(text_area.width.max(1));
        let view_row = clamp_coordinate(mouse.row, text_area.y, text_area.height);
        let view_col = clamp_coordinate(mouse.column, text_area.x, text_area.width);
        let scroll_row = self.semantic.buffer().scroll_row;
        if let Some((line_idx, byte_offset)) = text_hit_test(
            self.semantic.buffer(),
            view_width,
            scroll_row,
            usize::from(view_row),
            usize::from(view_col),
        ) {
            self.semantic
                .set_selection_anchor_if_absent((anchor_row, anchor_col));
            self.prepare_semantic_selection(true);
            self.semantic.set_cursor(line_idx, byte_offset);
            self.clamp_semantic_scroll_to_cursor();
        }
    }

    pub(super) fn prepare_scroll_drag(
        &self,
        target: ScrollTarget,
        axis: ScrollbarAxis,
        context: ScrollbarContext,
        column: u16,
        row: u16,
    ) -> Option<ScrollDrag> {
        if context.max_scroll == 0 {
            return None;
        }
        let (extent, relative, scroll_position) = match axis {
            ScrollbarAxis::Vertical => {
                if context.track.height <= 1 {
                    return None;
                }
                let pointer = clamp_coordinate(row, context.track.y, context.track.height);
                let scroll_position = match target {
                    ScrollTarget::Results => self.results.scroll_row().min(context.max_scroll),
                    ScrollTarget::Schema => self.schema.scroll_offset().min(context.max_scroll),
                };
                (context.track.height, pointer, scroll_position)
            }
            ScrollbarAxis::Horizontal => {
                if context.track.width <= 1 {
                    return None;
                }
                if !matches!(target, ScrollTarget::Results) {
                    return None;
                }
                let pointer = clamp_coordinate(column, context.track.x, context.track.width);
                let scroll_position = self.results.scroll_col().min(context.max_scroll);
                (context.track.width, pointer, scroll_position)
            }
        };
        if extent <= 1 {
            return None;
        }
        let thumb_coord = track_coordinate_for_scroll(scroll_position, extent, context.max_scroll);
        let grab_offset = i32::from(relative) - i32::from(thumb_coord);
        Some(ScrollDrag {
            axis,
            context,
            grab_offset,
            target,
        })
    }

    pub(super) fn apply_scroll_drag_at(
        &mut self,
        drag: &ScrollDrag,
        column: u16,
        row: u16,
        dragging: bool,
    ) {
        let context = &drag.context;
        if context.max_scroll == 0 {
            return;
        }
        match drag.axis {
            ScrollbarAxis::Vertical => {
                if context.track.height <= 1 {
                    return;
                }
                let pointer = clamp_coordinate(row, context.track.y, context.track.height);
                let relative =
                    apply_drag_offset(pointer, drag.grab_offset, context.track.height, dragging);
                let top_index =
                    scaled_scrollbar_position(relative, context.track.height, context.max_scroll);
                match drag.target {
                    ScrollTarget::Results => self.results.set_manual_scroll_row(top_index),
                    ScrollTarget::Schema => {
                        let viewport = self.schema_viewport_rows.max(1);
                        self.schema.set_manual_scroll_top(top_index, viewport);
                    }
                }
            }
            ScrollbarAxis::Horizontal => {
                if context.track.width <= 1 {
                    return;
                }
                let pointer = clamp_coordinate(column, context.track.x, context.track.width);
                let relative =
                    apply_drag_offset(pointer, drag.grab_offset, context.track.width, dragging);
                let top_index =
                    scaled_scrollbar_position(relative, context.track.width, context.max_scroll);
                if matches!(drag.target, ScrollTarget::Results) {
                    self.results.set_manual_scroll_col(top_index);
                }
            }
        }
    }
}

fn clamp_coordinate(value: u16, origin: u16, extent: u16) -> u16 {
    if extent == 0 {
        return 0;
    }
    if value <= origin {
        0
    } else if value >= origin.saturating_add(extent).saturating_sub(1) {
        extent.saturating_sub(1)
    } else {
        value - origin
    }
}

fn scaled_scrollbar_position(relative: u16, extent: u16, max_scroll: usize) -> usize {
    if max_scroll == 0 || extent <= 1 {
        return 0;
    }
    let span = extent.saturating_sub(1).max(1);
    let track_span = usize::from(span);
    if track_span == 0 {
        return 0;
    }
    let clamped = usize::from(relative.min(span));
    (clamped.saturating_mul(max_scroll) + track_span / 2) / track_span
}

fn track_coordinate_for_scroll(top_index: usize, extent: u16, max_scroll: usize) -> u16 {
    if max_scroll == 0 || extent <= 1 {
        return 0;
    }
    let span = extent.saturating_sub(1).max(1);
    let track_span = usize::from(span);
    if track_span == 0 {
        return 0;
    }
    let clamped_top = top_index.min(max_scroll);
    let numerator = clamped_top.saturating_mul(track_span);
    let coord = (numerator + max_scroll / 2) / max_scroll;
    u16::try_from(coord).unwrap_or(u16::MAX)
}

fn apply_drag_offset(relative: u16, offset: i32, extent: u16, dragging: bool) -> u16 {
    if extent <= 1 {
        return 0;
    }
    let span = extent.saturating_sub(1);
    let mut coord = i32::from(relative) - offset;
    let max_coord = i32::from(span);
    if dragging {
        if relative == 0 {
            coord = 0;
        } else if relative == span {
            coord = max_coord;
        }
    }
    if coord < 0 {
        coord = 0;
    } else if coord > max_coord {
        coord = max_coord;
    }
    u16::try_from(coord).unwrap_or_default()
}
