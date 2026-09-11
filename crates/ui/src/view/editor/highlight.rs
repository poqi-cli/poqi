use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    app::App,
    state::{editor_syntax::SyntaxKind, text_input::SelectionRange},
    view::text_input::{selection_span_for_row as text_selection_span, WrappedRow},
};

use crate::state::text_input::take_chars;

impl App {
    pub(super) fn highlight_line(
        &self,
        row: &WrappedRow,
        selection: Option<(usize, usize)>,
    ) -> Line<'static> {
        if row.text.is_empty() {
            let style = selection.map_or(self.theme.text, |_| self.theme.editor_selection);
            return Line::from(Span::styled(String::new(), style));
        }
        let spans = self.syntax.highlight(row.text.as_str());
        if spans.is_empty() {
            let style = selection.map_or(self.theme.text, |_| self.theme.editor_selection);
            return Line::from(Span::styled(row.text.clone(), style));
        }
        let mut rendered = Vec::new();
        let mut cursor = 0usize;
        for segment in spans {
            let seg_style = self.syntax_style(segment.kind);
            let seg_len = segment.text.chars().count();
            if let Some((sel_start, sel_end)) = selection {
                let seg_start = cursor;
                let seg_end = cursor + seg_len;
                if sel_end <= seg_start || sel_start >= seg_end {
                    rendered.push(Span::styled(segment.text, seg_style));
                } else {
                    let overlap_start = sel_start.max(seg_start);
                    let overlap_end = sel_end.min(seg_end);
                    let prefix_len = overlap_start.saturating_sub(seg_start);
                    if prefix_len > 0 {
                        let prefix = take_chars(&segment.text, 0, prefix_len);
                        if !prefix.is_empty() {
                            rendered.push(Span::styled(prefix, seg_style));
                        }
                    }
                    let selected_len = overlap_end.saturating_sub(overlap_start);
                    if selected_len > 0 {
                        let offset = overlap_start.saturating_sub(seg_start);
                        let selected = take_chars(&segment.text, offset, selected_len);
                        if !selected.is_empty() {
                            rendered.push(Span::styled(selected, self.theme.editor_selection));
                        }
                    }
                    let suffix_len = seg_end.saturating_sub(overlap_end);
                    if suffix_len > 0 {
                        let offset = overlap_end.saturating_sub(seg_start);
                        let suffix = take_chars(&segment.text, offset, suffix_len);
                        if !suffix.is_empty() {
                            rendered.push(Span::styled(suffix, seg_style));
                        }
                    }
                }
            } else {
                rendered.push(Span::styled(segment.text, seg_style));
            }
            cursor += seg_len;
        }
        Line::from(rendered)
    }

    pub(super) fn syntax_style(&self, kind: SyntaxKind) -> Style {
        match kind {
            SyntaxKind::Keyword => self.theme.syntax_keyword,
            SyntaxKind::Identifier => self.theme.syntax_identifier,
            SyntaxKind::Literal => self.theme.syntax_literal,
            SyntaxKind::Number => self.theme.syntax_number,
            SyntaxKind::Comment => self.theme.syntax_comment,
            SyntaxKind::Operator => self.theme.syntax_operator,
            SyntaxKind::Whitespace => self.theme.text,
        }
    }

    pub(super) fn selection_span_for_row(
        &self,
        row: &WrappedRow,
        selection: &SelectionRange,
    ) -> Option<(usize, usize)> {
        if row.text.is_empty() {
            return None;
        }
        text_selection_span(&self.editor, row, selection)
    }
}
