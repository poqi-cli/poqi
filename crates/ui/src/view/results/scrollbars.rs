use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ScrollbarAxis, ScrollbarContext};

use super::text::clip_text_to_width;

#[derive(Clone, Copy)]
pub(super) struct HorizontalOverflowHints {
    pub(super) left: bool,
    pub(super) right: bool,
}

#[derive(Clone, Copy)]
enum OverflowDirection {
    Left,
    Right,
}

impl App {
    pub(crate) fn render_scrollbar_with_context(
        frame: &mut Frame<'_>,
        ctx: &ScrollbarContext,
        axis: ScrollbarAxis,
        scroll_value: usize,
        track_style: Style,
        thumb_style: Style,
    ) {
        render_scrollbar_with_context(frame, ctx, axis, scroll_value, track_style, thumb_style);
    }

    pub(super) fn draw_results_scrollbar(
        &mut self,
        frame: &mut Frame<'_>,
        content_area: Rect,
        header_height: u16,
        body_height: u16,
        viewport_rows: usize,
    ) {
        let total_rows = self.results.row_count();
        if body_height == 0 || total_rows <= viewport_rows {
            return;
        }
        let track = Rect {
            x: content_area
                .x
                .saturating_add(content_area.width.saturating_sub(1)),
            y: content_area.y.saturating_add(header_height),
            width: 1,
            height: body_height,
        };
        if track.height == 0 {
            return;
        }
        self.results_vertical_scrollbar = Some(ScrollbarContext {
            track,
            viewport_items: viewport_rows,
            max_scroll: total_rows.saturating_sub(viewport_rows),
        });
        self.render_custom_scrollbar(frame, ScrollbarAxis::Vertical);
    }

    pub(super) fn draw_results_horizontal_scrollbar(
        &mut self,
        frame: &mut Frame<'_>,
        content_area: Rect,
        has_vertical_scrollbar: bool,
        viewport_columns: usize,
        overflow: HorizontalOverflowHints,
        show_track: bool,
    ) {
        if content_area.height == 0 {
            return;
        }
        let mut track_width = content_area.width;
        if has_vertical_scrollbar {
            track_width = track_width.saturating_sub(1);
        }
        if track_width == 0 {
            return;
        }
        let track = Rect {
            x: content_area.x,
            y: content_area.y.saturating_add(content_area.height),
            width: track_width,
            height: 1,
        };
        let max_scroll = self.results.max_horizontal_scroll();
        if max_scroll == 0 {
            return;
        }
        if show_track {
            self.results_horizontal_scrollbar = Some(ScrollbarContext {
                track,
                viewport_items: viewport_columns,
                max_scroll,
            });
            self.render_custom_scrollbar(frame, ScrollbarAxis::Horizontal);
        } else {
            self.results_horizontal_scrollbar = None;
            self.render_horizontal_track_background(frame, track);
        }
        self.draw_horizontal_overflow_markers(frame, track, overflow);
    }

    pub(super) fn render_custom_scrollbar(&self, frame: &mut Frame<'_>, axis: ScrollbarAxis) {
        let (context, scroll_value, track_style, thumb_style) = match axis {
            ScrollbarAxis::Vertical => (
                self.results_vertical_scrollbar.as_ref(),
                self.results.scroll_row(),
                self.theme.panel_background_selected,
                self.theme.accent_secondary,
            ),
            ScrollbarAxis::Horizontal => (
                self.results_horizontal_scrollbar.as_ref(),
                self.results.scroll_col(),
                self.theme.panel_background,
                self.theme.accent_secondary,
            ),
        };
        let Some(ctx) = context else {
            return;
        };
        render_scrollbar_with_context(frame, ctx, axis, scroll_value, track_style, thumb_style);
    }
}

fn render_scrollbar_with_context(
    frame: &mut Frame<'_>,
    ctx: &ScrollbarContext,
    axis: ScrollbarAxis,
    scroll_value: usize,
    track_style: Style,
    thumb_style: Style,
) {
    let viewport = ctx.viewport_items.max(1);
    let max_scroll = ctx.max_scroll;
    match axis {
        ScrollbarAxis::Vertical => {
            let track_len = usize::from(ctx.track.height);
            if track_len == 0 {
                return;
            }
            let (offset, size) = scrollbar_thumb(track_len, viewport, max_scroll, scroll_value);
            let mut lines = Vec::with_capacity(track_len);
            for idx in 0..track_len {
                let style = if idx >= offset && idx < offset + size {
                    thumb_style
                } else {
                    track_style
                };
                let glyph = if idx >= offset && idx < offset + size {
                    "█"
                } else {
                    "│"
                };
                lines.push(Line::from(Span::styled(glyph, style)));
            }
            frame.render_widget(Paragraph::new(lines), ctx.track);
        }
        ScrollbarAxis::Horizontal => {
            let track_len = usize::from(ctx.track.width);
            if track_len == 0 {
                return;
            }
            let (offset, size) = scrollbar_thumb(track_len, viewport, max_scroll, scroll_value);
            let mut spans = Vec::with_capacity(track_len);
            for idx in 0..track_len {
                let style = if idx >= offset && idx < offset + size {
                    thumb_style
                } else {
                    track_style
                };
                let glyph = if idx >= offset && idx < offset + size {
                    "█"
                } else {
                    "─"
                };
                spans.push(Span::styled(glyph, style));
            }
            let line = Line::from(spans);
            frame.render_widget(Paragraph::new(line), ctx.track);
        }
    }
}

impl App {
    fn draw_horizontal_overflow_markers(
        &self,
        frame: &mut Frame<'_>,
        track: Rect,
        overflow: HorizontalOverflowHints,
    ) {
        if track.width == 0 || track.height == 0 {
            return;
        }
        let marker_style = self.theme.text_muted.add_modifier(Modifier::BOLD);
        let label_style = self.theme.panel_background.patch(marker_style);
        if overflow.left {
            render_overflow_label(frame, track, OverflowDirection::Left, label_style);
        }
        if overflow.right {
            render_overflow_label(frame, track, OverflowDirection::Right, label_style);
        }
    }

    fn render_horizontal_track_background(&self, frame: &mut Frame<'_>, track: Rect) {
        if track.width == 0 || track.height == 0 {
            return;
        }
        let glyphs = "─".repeat(track.width as usize);
        let line = Line::from(Span::styled(glyphs, self.theme.panel_background));
        frame.render_widget(Paragraph::new(line), track);
    }
}

fn render_overflow_label(
    frame: &mut Frame<'_>,
    track: Rect,
    direction: OverflowDirection,
    style: Style,
) {
    if track.width == 0 {
        return;
    }
    let label = match direction {
        OverflowDirection::Left => "← more columns",
        OverflowDirection::Right => "more columns →",
    };
    let available = usize::from(track.width);
    let raw_width = UnicodeWidthStr::width(label).min(available);
    let Ok(label_width) = u16::try_from(raw_width) else {
        return;
    };
    if label_width == 0 {
        return;
    }
    let text = clip_text_to_width(label, label_width);
    let label_rect = match direction {
        OverflowDirection::Left => Rect {
            x: track.x,
            y: track.y,
            width: label_width,
            height: 1,
        },
        OverflowDirection::Right => Rect {
            x: track
                .x
                .saturating_add(track.width.saturating_sub(label_width)),
            y: track.y,
            width: label_width,
            height: 1,
        },
    };
    frame.render_widget(Paragraph::new(Span::styled(text, style)), label_rect);
}

fn scrollbar_thumb(
    track_len: usize,
    viewport_items: usize,
    max_scroll: usize,
    scroll_value: usize,
) -> (usize, usize) {
    if track_len == 0 {
        return (0, 0);
    }
    if max_scroll == 0 {
        return (0, track_len);
    }
    let viewport = viewport_items.max(1);
    let total = viewport.saturating_add(max_scroll).max(1);
    let midpoint = usize::midpoint(viewport, max_scroll);
    let thumb_len = ((track_len * viewport).max(1) + midpoint)
        .saturating_div(total)
        .max(1)
        .min(track_len);
    let travel = track_len.saturating_sub(thumb_len);
    if travel == 0 {
        return (0, track_len);
    }
    let clamped_scroll = scroll_value.min(max_scroll);
    let offset = (clamped_scroll * travel + max_scroll / 2) / max_scroll;
    let max_offset = track_len.saturating_sub(thumb_len);
    (offset.min(max_offset), thumb_len)
}
