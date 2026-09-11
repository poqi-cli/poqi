use crate::{app::text_nav, app::App, view::text_input::wrapped_content};

impl App {
    pub(crate) fn refresh_semantic_post_edit(&mut self) {
        self.clamp_semantic_scroll_to_cursor();
    }

    pub(crate) fn move_semantic_cursor_vertical(
        &mut self,
        delta: isize,
        step: usize,
        extend: bool,
    ) {
        text_nav::move_cursor_vertical(&mut self.semantic, delta, step, extend);
        self.clamp_semantic_scroll_to_cursor();
    }

    pub(crate) fn move_semantic_cursor_horizontal(
        &mut self,
        delta: isize,
        step: usize,
        extend: bool,
    ) {
        text_nav::move_cursor_horizontal(&mut self.semantic, delta, step, extend);
        self.clamp_semantic_scroll_to_cursor();
    }

    pub(crate) fn prepare_semantic_selection(&mut self, extend: bool) {
        self.semantic.prepare_selection(extend);
    }

    pub(crate) fn clamp_semantic_scroll_to_cursor(&mut self) {
        let Some(area) = self.semantic_text_area else {
            return;
        };
        if area.width == 0 || area.height == 0 {
            return;
        }
        let view_width = usize::from(area.width.max(1));
        let view_height = usize::from(area.height.max(1));
        let (wrapped_rows, cursor_visual_row, _) =
            wrapped_content(self.semantic.buffer(), view_width);
        self.semantic
            .ensure_cursor_visible(cursor_visual_row, view_height, wrapped_rows.len());
    }
}
