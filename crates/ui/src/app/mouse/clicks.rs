use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::{
    geometry::point_in_rect, input::FocusPanel, state::schema::SchemaItem,
    view::text_input::hit_test as text_hit_test,
};

use super::super::{App, ScrollTarget, ScrollbarAxis};

impl App {
    pub(super) fn handle_mouse_click(&mut self, mouse: MouseEvent) {
        if let Some(panel) = self.panel_under_pointer(mouse) {
            if panel != FocusPanel::Editor {
                self.clear_editor_drag_anchor();
            }
            if panel != FocusPanel::SemanticSearch {
                self.clear_semantic_drag_anchor();
            }
            self.focus_window(panel);
            match panel {
                FocusPanel::Schema => {
                    if self.handle_schema_scrollbar_pointer(mouse) {
                        return;
                    }
                    self.handle_schema_click(mouse);
                }
                FocusPanel::Editor => self.handle_editor_click(mouse),
                FocusPanel::SemanticSearch => self.handle_semantic_click(mouse),
                FocusPanel::Results => {
                    if self.handle_results_scrollbar_pointer(mouse) {
                        return;
                    }
                    self.handle_results_click(mouse);
                }
                FocusPanel::Status => {
                    self.open_settings_modal();
                }
            }
        } else {
            self.clear_editor_drag_anchor();
            self.clear_semantic_drag_anchor();
        }
    }

    pub(super) fn handle_semantic_click(&mut self, mouse: MouseEvent) {
        if let Some(area) = self.semantic_text_area {
            let within_y = mouse.row >= area.y && mouse.row < area.y.saturating_add(area.height);
            let within_x =
                mouse.column >= area.x && mouse.column < area.x.saturating_add(area.width);
            if within_x && within_y {
                let view_row = usize::from(mouse.row.saturating_sub(area.y));
                let view_col = usize::from(mouse.column.saturating_sub(area.x));
                let view_width = usize::from(area.width.max(1));
                let scroll_row = self.semantic.buffer().scroll_row;
                let previous_cursor = self.semantic.buffer().cursor_position();
                if let Some((line_idx, byte_offset)) = text_hit_test(
                    self.semantic.buffer(),
                    view_width,
                    scroll_row,
                    view_row,
                    view_col,
                ) {
                    let extend = mouse.modifiers.contains(KeyModifiers::SHIFT);
                    self.prepare_semantic_selection(extend);
                    self.semantic.set_cursor(line_idx, byte_offset);
                    self.clamp_semantic_scroll_to_cursor();
                    let anchor = if extend {
                        self.semantic
                            .buffer()
                            .selection_anchor
                            .unwrap_or(previous_cursor)
                    } else {
                        self.semantic.buffer().cursor_position()
                    };
                    self.semantic_mouse_drag_anchor = Some(anchor);
                    return;
                }
            }
        }
        self.semantic_mouse_drag_anchor = None;
        let hint = self.semantic.next_hint().to_string();
        self.status.info(format!("Semantic search: {hint}"));
    }

    fn handle_schema_click(&mut self, mouse: MouseEvent) {
        if let Some(area) = self.schema_area {
            let content_y = area.y + 1;
            let content_x = area.x + 1;
            if mouse.row >= content_y
                && mouse.row < area.y + area.height - 1
                && mouse.column >= content_x
                && mouse.column < area.x + area.width - 1
            {
                let visible_row = usize::from(mouse.row.saturating_sub(content_y));
                let clicked_index = visible_row.saturating_add(self.schema.scroll_offset());
                if clicked_index < self.schema.items.len() {
                    let item = self.schema.items.get(clicked_index).cloned();
                    self.schema.select_index(clicked_index);
                    if let Some(item) = item {
                        match item {
                            SchemaItem::Schema { .. } => {
                                if self.schema.drill_down() {
                                    self.auto_fetch_on_table_selection();
                                }
                            }
                            SchemaItem::Table { .. } => {
                                self.auto_fetch_on_table_selection();
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_editor_click(&mut self, mouse: MouseEvent) {
        if self.handle_completion_popup_click(mouse) {
            return;
        }
        let Some(text_area) = self.editor_text_area else {
            return;
        };
        let within_y =
            mouse.row >= text_area.y && mouse.row < text_area.y.saturating_add(text_area.height);
        let within_x = mouse.column >= text_area.x
            && mouse.column < text_area.x.saturating_add(text_area.width);
        if !within_x || !within_y {
            return;
        }
        let row = usize::from(mouse.row.saturating_sub(text_area.y));
        let col = usize::from(mouse.column.saturating_sub(text_area.x));
        let view_width = usize::from(text_area.width.max(1));
        let gutter_width = self.editor_gutter_width().min(view_width);
        let gutter_click = col < gutter_width;
        let extend = mouse.modifiers.contains(KeyModifiers::SHIFT);
        let previous_cursor = self.editor.cursor_position();
        let absolute_visual_row = self.editor.scroll_row.saturating_add(row);
        let total_rows = self.editor_row_count(view_width);
        if gutter_click || absolute_visual_row >= total_rows {
            let desired_row = absolute_visual_row;
            self.editor.ensure_line_count(desired_row.saturating_add(1));
            let target_col = if gutter_click {
                let (_, prev_col) = previous_cursor;
                self.editor
                    .lines
                    .get(desired_row)
                    .map_or(0, |line| prev_col.min(line.len()))
            } else {
                let content_col = col.saturating_sub(gutter_width);
                self.editor
                    .lines
                    .get(desired_row)
                    .map_or(0, |line| content_col.min(line.len()))
            };
            self.prepare_editor_selection(extend);
            self.editor.set_cursor(desired_row, target_col);
            self.editor_mouse_drag_anchor = Some(if extend {
                self.editor.selection_anchor.unwrap_or(previous_cursor)
            } else {
                self.editor.cursor_position()
            });
            self.refresh_editor_after_cursor_move();
            return;
        }
        if let Some((line_idx, byte_offset)) = self.editor_hit_test(view_width, row, col) {
            let (clamped_row, clamped_col) = self.clamp_editor_hit(line_idx, byte_offset);
            self.prepare_editor_selection(extend);
            self.editor.set_cursor(clamped_row, clamped_col);
            self.editor_mouse_drag_anchor = Some(if extend {
                self.editor.selection_anchor.unwrap_or(previous_cursor)
            } else {
                self.editor.cursor_position()
            });
            self.refresh_editor_after_cursor_move();
        } else {
            self.prepare_editor_selection(extend);
            self.editor.move_to_end();
            self.editor_mouse_drag_anchor = None;
            self.refresh_editor_after_cursor_move();
        }
    }

    fn handle_results_click(&mut self, mouse: MouseEvent) {
        let extend = mouse.modifiers.contains(KeyModifiers::SHIFT);
        if self.try_results_hitbox_focus(mouse.column, mouse.row, extend) {
            return;
        }
        self.focus_results_list_row(mouse.row, extend);
    }

    fn handle_results_scrollbar_pointer(&mut self, mouse: MouseEvent) -> bool {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return false;
        }
        if let Some(ctx) = self.results_vertical_scrollbar.clone() {
            if point_in_rect(mouse.column, mouse.row, &ctx.track) {
                if let Some(drag) = self.prepare_scroll_drag(
                    ScrollTarget::Results,
                    ScrollbarAxis::Vertical,
                    ctx,
                    mouse.column,
                    mouse.row,
                ) {
                    self.apply_scroll_drag_at(&drag, mouse.column, mouse.row, false);
                    self.scroll_drag = Some(drag);
                    return true;
                }
            }
        }
        if let Some(ctx) = self.results_horizontal_scrollbar.clone() {
            if point_in_rect(mouse.column, mouse.row, &ctx.track) {
                if let Some(drag) = self.prepare_scroll_drag(
                    ScrollTarget::Results,
                    ScrollbarAxis::Horizontal,
                    ctx,
                    mouse.column,
                    mouse.row,
                ) {
                    self.apply_scroll_drag_at(&drag, mouse.column, mouse.row, false);
                    self.scroll_drag = Some(drag);
                    return true;
                }
            }
        }
        false
    }

    fn handle_schema_scrollbar_pointer(&mut self, mouse: MouseEvent) -> bool {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return false;
        }
        if let Some(ctx) = self.schema_scrollbar.clone() {
            if point_in_rect(mouse.column, mouse.row, &ctx.track) {
                if let Some(drag) = self.prepare_scroll_drag(
                    ScrollTarget::Schema,
                    ScrollbarAxis::Vertical,
                    ctx,
                    mouse.column,
                    mouse.row,
                ) {
                    self.apply_scroll_drag_at(&drag, mouse.column, mouse.row, false);
                    self.scroll_drag = Some(drag);
                    return true;
                }
            }
        }
        false
    }

    fn try_results_hitbox_focus(&mut self, column: u16, row: u16, extend: bool) -> bool {
        if let Some(hitbox) = &self.results_hitbox {
            if let Some((cell_row, cell_col)) =
                hitbox.hit_test(column, row, self.results.row_count())
            {
                self.results.set_focus(cell_row, cell_col, extend);
                return true;
            }
        }
        false
    }

    fn focus_results_list_row(&mut self, row: u16, extend: bool) {
        if let Some(area) = self.results_area {
            let content_y = area.y + 2;
            if row >= content_y && row < area.y + area.height - 1 {
                let visible_index = (row - content_y) as usize;
                let target_row = visible_index.saturating_add(self.results.scroll_row());
                if target_row < self.results.row_count() {
                    self.results
                        .set_focus(target_row, self.results.focus_cell.1, extend);
                }
            }
        }
    }

    fn handle_completion_popup_click(&mut self, mouse: MouseEvent) -> bool {
        let Some(hitbox) = self.completion_popup_hitbox else {
            return false;
        };
        if !self.editor.completions_visible() {
            return false;
        }
        if !point_in_rect(mouse.column, mouse.row, &hitbox.content_area) {
            return false;
        }
        if hitbox.visible_rows == 0 {
            return false;
        }
        let relative_row = mouse.row.saturating_sub(hitbox.content_area.y);
        let content_limit = hitbox.content_area.height.saturating_sub(1);
        let row = usize::from(relative_row.min(content_limit));
        let clamped_row = row.min(hitbox.visible_rows.saturating_sub(1));
        let target_index = hitbox.start_index.saturating_add(clamped_row);
        self.editor.select_completion_index(target_index);
        if self.editor.accept_selected_completion() {
            self.refresh_editor_completions();
        }
        true
    }
}
