use poqi_search_completion::{CompletionBatch, CompletionItem, CompletionKind};

use super::EditorState;

impl EditorState {
    pub(crate) fn set_completions(&mut self, batch: CompletionBatch) {
        self.completion_popup.set_items(batch.items);
        self.needs_completion_refresh = false;
    }

    pub(crate) fn completion_popup_items(&self) -> (&[CompletionItem], Option<usize>) {
        (
            self.completion_popup.items(),
            self.completion_popup.selected_index(),
        )
    }

    pub(crate) fn completion_visible_range(&self) -> (usize, usize) {
        self.completion_popup.visible_range()
    }

    pub(crate) fn set_completion_viewport_rows(&mut self, rows: usize) {
        self.completion_popup.set_viewport_rows(rows);
    }

    pub(crate) fn scroll_completion_popup(&mut self, delta: isize) {
        self.completion_popup.scroll_viewport(delta);
    }

    pub(crate) fn select_completion_index(&mut self, index: usize) {
        self.completion_popup.set_selected_index(index);
    }

    pub(crate) fn clear_completions(&mut self) {
        self.completion_popup.clear();
        self.needs_completion_refresh = false;
    }

    pub(crate) fn maybe_commit_completion(&mut self) -> bool {
        // Auto-commit zero-score completions so we finish exact matches without extra keystrokes.
        if let Some((insert_text, kind)) = self
            .completion_popup
            .current()
            .filter(|candidate| candidate.score == 0)
            .map(|item| (item.insert_text.clone(), item.kind))
        {
            self.insert_completion_text(&insert_text, kind);
            return true;
        }
        false
    }

    pub(crate) fn mark_completions_dirty(&mut self) {
        self.needs_completion_refresh = true;
    }

    pub(crate) fn completions_visible(&self) -> bool {
        self.completion_popup.is_visible()
    }

    pub(crate) fn select_next_completion(&mut self) {
        self.completion_popup.select_next();
    }

    pub(crate) fn select_previous_completion(&mut self) {
        self.completion_popup.select_previous();
    }

    pub(crate) fn accept_selected_completion(&mut self) -> bool {
        if let Some((text, kind)) = self
            .completion_popup
            .current()
            .map(|item| (item.insert_text.clone(), item.kind))
        {
            self.insert_completion_text(&text, kind);
            return true;
        }
        false
    }

    pub(crate) fn replace_current_token(&mut self, replacement: &str) {
        if self.cursor_row >= self.lines.len() {
            return;
        }
        if let Some(selection) = self.selection_range() {
            if selection.start.0 != selection.end.0 {
                self.collapse_selection();
                self.insert_text(replacement);
                return;
            }
        }
        let row = self.cursor_row;
        let cursor_col = self.cursor_col;
        let selection = self.selection_range();
        let line = &mut self.lines[row];
        let cursor = cursor_col.min(line.len());
        let (start, end) = selection.map_or_else(
            || identifier_component_at(line, cursor).unwrap_or((cursor, cursor)),
            |range| {
                identifier_component_containing(line, range.start.1, range.end.1)
                    .unwrap_or((range.start.1, range.end.1))
            },
        );
        line.replace_range(start..end, replacement);
        self.cursor_col = start + replacement.len();
        self.selection_anchor = None;
        self.needs_completion_refresh = true;
    }

    fn insert_completion_text(&mut self, text: &str, kind: CompletionKind) {
        if kind == CompletionKind::Keyword && self.after_closed_quoted_identifier() {
            self.insert_text(&format!(" {text}"));
        } else {
            self.replace_current_token(text);
        }
        self.clear_completions();
        self.needs_completion_refresh = text.ends_with('.') || kind == CompletionKind::Table;
    }

    fn after_closed_quoted_identifier(&self) -> bool {
        if self.selection_range().is_some() {
            return false;
        }
        let Some(line) = self.lines.get(self.cursor_row) else {
            return false;
        };
        let Some((start, end)) = identifier_component_at(line, self.cursor_col) else {
            return false;
        };
        let component = &line[start..end];
        end == self.cursor_col
            && component.starts_with('"')
            && component.ends_with('"')
            && component.bytes().filter(|byte| *byte == b'"').count() % 2 == 0
    }
}

fn identifier_component_at(line: &str, cursor: usize) -> Option<(usize, usize)> {
    identifier_components(line)
        .into_iter()
        .find(|(start, end)| *start < cursor && cursor <= *end)
}

fn identifier_component_containing(
    line: &str,
    selection_start: usize,
    selection_end: usize,
) -> Option<(usize, usize)> {
    identifier_components(line)
        .into_iter()
        .find(|(start, end)| *start <= selection_start && selection_end <= *end)
}

fn identifier_components(line: &str) -> Vec<(usize, usize)> {
    let mut components = Vec::new();
    let mut idx = 0;
    while idx < line.len() {
        let Some(ch) = line[idx..].chars().next() else {
            break;
        };
        if ch == '"' {
            let start = idx;
            idx += 1;
            while idx < line.len() {
                let Some(current) = line[idx..].chars().next() else {
                    break;
                };
                idx += current.len_utf8();
                if current == '"' {
                    if line[idx..].starts_with('"') {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            components.push((start, idx));
            continue;
        }
        if identifier_char(ch) {
            let start = idx;
            idx += ch.len_utf8();
            while idx < line.len() {
                let Some(current) = line[idx..].chars().next() else {
                    break;
                };
                if !identifier_char(current) {
                    break;
                }
                idx += current.len_utf8();
            }
            components.push((start, idx));
            continue;
        }
        idx += ch.len_utf8();
    }
    components
}

fn identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}
