use poqi_search_completion::{CompletionBatch, CompletionItem, CompletionKind};

use super::EditorState;

impl EditorState {
    pub(crate) fn set_selection_for_test(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        let anchor = self.clamp_position(anchor);
        self.set_cursor(head.0, head.1);
        if anchor == self.cursor_position() {
            self.selection_anchor = None;
        } else {
            self.selection_anchor = Some(anchor);
        }
    }
}

#[test]
fn replacing_qualified_table_does_not_duplicate_schema() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT * FROM schema1.table".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.replace_current_token("table_10");
    assert_eq!(editor.lines[0].as_str(), "SELECT * FROM schema1.table_10");
}

#[test]
fn completion_replaces_entire_identifier_at_middle_cursor() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT".to_string()];
    editor.set_cursor(0, 3);

    editor.replace_current_token("SELECT");

    assert_eq!(editor.lines[0], "SELECT");
    assert_eq!(editor.cursor_position(), (0, "SELECT".len()));
}

#[test]
fn completion_at_identifier_start_does_not_replace_following_keyword() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT FROM public.widgets".to_string()];
    editor.set_cursor(0, "SELECT ".len());

    editor.replace_current_token("*");

    assert_eq!(editor.lines[0], "SELECT *FROM public.widgets");
}

#[test]
fn accepted_completion_closes_popup_unless_it_chains() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT i".to_string()];
    let line_end = editor.lines[0].len();
    editor.set_cursor(0, line_end);
    editor.set_completions(completion_batch("id"));
    editor.select_completion_index(0);

    assert!(editor.accept_selected_completion());
    assert!(!editor.needs_completion_refresh);
    assert!(!editor.completions_visible());

    editor.set_completions(completion_batch("public."));
    editor.select_completion_index(0);
    assert!(editor.accept_selected_completion());
    assert!(editor.needs_completion_refresh);
}

fn completion_batch(insert_text: &str) -> CompletionBatch {
    CompletionBatch {
        items: std::iter::once(CompletionItem {
            label: insert_text.to_string(),
            insert_text: insert_text.to_string(),
            detail: String::new(),
            kind: CompletionKind::Keyword,
            score: 1,
        })
        .collect(),
    }
}

#[test]
fn accepting_table_then_where_keeps_the_qualified_relation() {
    use poqi_catalog::{CatalogSnapshot, RelationKind, TableId, TableMeta};
    use poqi_search_completion::CompletionService;

    let service = CompletionService::new(CatalogSnapshot {
        tables: vec![TableMeta {
            id: TableId("schema2.addresses_10".into()),
            schema: "schema2".into(),
            name: "addresses_10".into(),
            relation_kind: RelationKind::Table,
        }],
        ..CatalogSnapshot::default()
    });
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT * FROM \"schema2\".add".into()];
    let line_end = editor.lines[0].len();
    editor.set_cursor(0, line_end);
    let batch = service.suggest(&editor.lines[0], editor.cursor_col);
    let table_index = batch
        .items
        .iter()
        .position(|item| item.insert_text == "\"addresses_10\"")
        .expect("table suggestion");
    editor.set_completions(batch);
    editor.select_completion_index(table_index);
    assert!(editor.accept_selected_completion());
    assert_eq!(
        editor.lines[0],
        "SELECT * FROM \"schema2\".\"addresses_10\""
    );
    assert!(editor.needs_completion_refresh);

    let batch = service.suggest(&editor.lines[0], editor.cursor_col);
    let where_index = batch
        .items
        .iter()
        .position(|item| item.label == "WHERE")
        .expect("clause suggestion after accepting table");
    editor.set_completions(batch);
    editor.select_completion_index(where_index);
    assert!(editor.accept_selected_completion());
    assert_eq!(
        editor.lines[0],
        "SELECT * FROM \"schema2\".\"addresses_10\" WHERE"
    );
}

#[test]
fn clause_completion_preserves_closed_quoted_identifiers_and_whitespace() {
    for source in [
        "SELECT * FROM \"schema2\".\"addresses_10\"",
        "SELECT * FROM \"schema2\".\"addresses_10\" ",
        "SELECT * FROM \"odd\"\"name\"",
    ] {
        let mut editor = EditorState::new();
        editor.lines = vec![source.into()];
        editor.set_cursor(0, source.len());
        editor.set_completions(completion_batch("WHERE"));
        editor.select_completion_index(0);
        assert!(editor.accept_selected_completion());
        assert_eq!(editor.lines[0], format!("{} WHERE", source.trim_end()));
    }
}

#[test]
fn completion_replaces_unicode_identifier_at_middle_cursor() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT äitix FROM t".to_string()];
    editor.set_cursor(0, "SELECT äi".len());

    editor.replace_current_token("\"äiti\"");

    assert_eq!(editor.lines[0], "SELECT \"äiti\" FROM t");
}

#[test]
fn completion_replaces_quoted_component_with_dots_and_escaped_quotes() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT \"schema.with.dot\".\"odd\"\"namex\" FROM t".to_string()];
    editor.set_cursor(0, "SELECT \"schema.with.dot\".\"odd\"\"na".len());

    editor.replace_current_token("\"odd\"\"name\"");

    assert_eq!(
        editor.lines[0],
        "SELECT \"schema.with.dot\".\"odd\"\"name\" FROM t"
    );
}

#[test]
fn completion_expands_single_line_selection_to_whole_identifier() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT".to_string()];
    editor.set_selection_for_test((0, 2), (0, 4));

    editor.replace_current_token("SELECT");

    assert_eq!(editor.lines[0], "SELECT");
    assert!(editor.selection_range().is_none());
}

#[test]
fn completion_replaces_multiline_selection_without_joining_neighbor_tokens() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT old".to_string(), "value FROM t".to_string()];
    editor.set_selection_for_test((0, 7), (1, 5));

    editor.replace_current_token("\"id\"");

    assert_eq!(editor.lines, vec!["SELECT \"id\" FROM t".to_string()]);
    assert!(editor.selection_range().is_none());
}

#[test]
fn replacing_unqualified_table_inserts_full_name() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT * FROM tab".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.replace_current_token("schema1.table_10");
    assert_eq!(editor.lines[0].as_str(), "SELECT * FROM schema1.table_10");
}

#[test]
fn insert_text_inserts_multi_line_content() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.insert_text(" *\nFROM\n table");
    assert_eq!(
        editor.lines,
        vec![
            "SELECT *".to_string(),
            "FROM".to_string(),
            " table".to_string()
        ]
    );
    assert_eq!(editor.cursor_row, 2);
    assert_eq!(editor.cursor_col, editor.lines[2].len());
}

#[test]
fn insert_text_preserves_line_tail() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SEL FROM table".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = 3;
    editor.insert_text("ECT\nfoo");
    assert_eq!(
        editor.lines,
        vec!["SELECT".to_string(), "foo FROM table".to_string()]
    );
    assert_eq!(editor.cursor_row, 1);
    assert_eq!(editor.cursor_col, "foo".len());
}

#[test]
fn insert_text_handles_multibyte_characters() {
    let mut editor = EditorState::new();
    editor.lines = vec!["SELECT".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.insert_text(" äiti\nhännällänsä");
    assert_eq!(
        editor.lines,
        vec!["SELECT äiti".to_string(), "hännällänsä".to_string()]
    );
    assert_eq!(editor.cursor_row, 1);
    assert_eq!(editor.cursor_col, "hännällänsä".len());
}

#[test]
fn backspace_handles_multibyte_characters() {
    let mut editor = EditorState::new();
    editor.lines = vec!["ää".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.backspace();
    assert_eq!(editor.lines[0].as_str(), "ä");
    editor.backspace();
    assert_eq!(editor.lines[0].as_str(), "");
    assert_eq!(editor.cursor_col, 0);
}

#[test]
fn scroll_by_clamps_within_bounds() {
    let mut editor = EditorState::new();
    editor.scroll_row = 5;
    editor.scroll_by(-10, 100, 5);
    assert_eq!(editor.scroll_row, 0);
    editor.scroll_by(999, 40, 5);
    assert_eq!(editor.scroll_row, 35);
}

#[test]
fn ensure_cursor_visible_updates_scroll_row() {
    let mut editor = EditorState::new();
    editor.scroll_row = 0;
    editor.ensure_cursor_visible(20, 5, 40);
    assert_eq!(editor.scroll_row, 16);
    editor.ensure_cursor_visible(2, 5, 40);
    assert_eq!(editor.scroll_row, 2);
}

#[test]
fn selection_range_normalizes_bounds() {
    let mut editor = EditorState::new();
    editor.lines = vec!["abcd".to_string(), "efgh".to_string()];
    editor.set_selection_for_test((1, 2), (0, 1));
    let range = editor.selection_range().expect("selection");
    assert_eq!(range.start, (0, 1));
    assert_eq!(range.end, (1, 2));
}

#[test]
fn edits_collapse_active_selection() {
    let mut editor = EditorState::new();
    editor.lines = vec!["abcdef".to_string()];
    editor.set_selection_for_test((0, 1), (0, 4));
    editor.insert_char('X');
    assert_eq!(editor.lines[0], "aXef");
    assert!(editor.selection_range().is_none());
    editor.set_selection_for_test((0, 1), (0, 2));
    editor.backspace();
    assert_eq!(editor.lines[0], "aef");
    assert!(editor.selection_range().is_none());
}

#[test]
fn tab_inserts_spaces_without_selection() {
    let mut editor = EditorState::new();
    editor.lines = vec![String::new()];
    editor.cursor_row = 0;
    editor.cursor_col = 0;
    editor.needs_completion_refresh = false;
    editor.insert_tab();
    assert_eq!(editor.lines[0].as_str(), "    ");
    assert_eq!(editor.cursor_col, 4);
    assert!(editor.needs_completion_refresh);
}

#[test]
fn tab_indents_selection_and_preserves_range() {
    let mut editor = EditorState::new();
    editor.lines = vec!["one".to_string(), "two".to_string()];
    editor.needs_completion_refresh = false;
    editor.set_selection_for_test((0, 0), (1, 3));
    editor.insert_tab();
    assert_eq!(editor.lines[0].as_str(), "    one");
    assert_eq!(editor.lines[1].as_str(), "    two");
    let range = editor.selection_range().expect("selection");
    assert_eq!(range.start, (0, 4));
    assert_eq!(range.end, (1, 7));
    assert!(editor.needs_completion_refresh);
}

#[test]
fn shift_tab_outdents_selection() {
    let mut editor = EditorState::new();
    editor.lines = vec!["    one".to_string(), "  two".to_string()];
    editor.needs_completion_refresh = false;
    editor.set_selection_for_test((0, 0), (1, 4));
    editor.outdent();
    assert_eq!(editor.lines[0].as_str(), "one");
    assert_eq!(editor.lines[1].as_str(), "two");
    let range = editor.selection_range().expect("selection");
    assert_eq!(range.start, (0, 0));
    assert_eq!(range.end, (1, 2));
    assert!(editor.needs_completion_refresh);
}

#[test]
fn shift_tab_outdents_current_line_without_selection() {
    let mut editor = EditorState::new();
    editor.lines = vec!["    select 1".to_string()];
    editor.cursor_row = 0;
    editor.cursor_col = editor.lines[0].len();
    editor.needs_completion_refresh = false;
    editor.outdent();
    assert_eq!(editor.lines[0].as_str(), "select 1");
    assert_eq!(editor.cursor_col, "select 1".len());
    assert!(editor.needs_completion_refresh);
}
