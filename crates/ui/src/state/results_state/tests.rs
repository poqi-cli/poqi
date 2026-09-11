use super::{EditCompletion, ResultsState};
use poqi_catalog::QualifiedRelation;
use poqi_engine::{ResultMetadata, ResultOrigin, RowIdentity};

#[test]
fn ensure_focus_col_visible_scrolls_when_needed() {
    let mut state = ResultsState::new();
    state.headers = vec!["c1".into(), "c2".into(), "c3".into()];
    state.rows = vec![vec!["a".into()]];
    state.set_focus(0, 2, false);
    state.ensure_focus_col_visible(5, 1, &[3, 3, 3]);
    assert_eq!(state.scroll_col, 2);
}

#[test]
fn ensure_focus_col_visible_clamps_when_width_zero() {
    let mut state = ResultsState::new();
    state.headers = vec!["c1".into()];
    state.rows = vec![vec!["a".into()]];
    state.ensure_focus_col_visible(0, 1, &[3]);
    assert_eq!(state.scroll_col, 0);
}

#[test]
fn horizontal_scrollbar_tracks_scroll_col() {
    let mut state = ResultsState::new();
    state.headers = vec!["c1".into(), "c2".into(), "c3".into(), "c4".into()];
    state.set_scroll_col(1);
    state.sync_horizontal_scrollbar(2);
    assert_eq!(state.horizontal_scrollbar_state().get_position(), 1);

    state.set_scroll_col(3);
    state.sync_horizontal_scrollbar(2);
    assert_eq!(state.horizontal_scrollbar_state().get_position(), 3);
}

#[test]
fn vertical_scrollbar_position_matches_top_index() {
    let mut state = ResultsState::new();
    state.headers = vec!["c".into()];
    state.rows = (0..50).map(|idx| vec![idx.to_string()]).collect();
    state.set_scroll_row(5);
    state.sync_scrollbar_thumb(4);
    assert_eq!(state.vertical_scrollbar_state().get_position(), 5);
    assert_eq!(state.scroll_row(), 5);

    state.set_scroll_row(40);
    state.sync_scrollbar_thumb(4);
    assert_eq!(state.vertical_scrollbar_state().get_position(), 40);
    assert_eq!(state.scroll_row(), 40);
}

#[test]
fn horizontal_scrollbar_position_matches_right_edge() {
    let mut state = ResultsState::new();
    state.headers = vec![
        "c1".into(),
        "c2".into(),
        "c3".into(),
        "c4".into(),
        "c5".into(),
    ];
    state.rows = vec![vec!["row".into()]];
    state.set_scroll_col(0);
    state.sync_horizontal_scrollbar(3);
    assert_eq!(state.horizontal_scrollbar_state().get_position(), 0);

    state.set_scroll_col(3);
    state.sync_horizontal_scrollbar(3);
    assert_eq!(state.horizontal_scrollbar_state().get_position(), 3);
    assert_eq!(state.scroll_col(), 3);
}

#[test]
fn manual_vertical_scroll_can_hide_focus() {
    let mut state = ResultsState::new();
    state.headers = vec!["c".into()];
    state.rows = (0..100).map(|idx| vec![idx.to_string()]).collect();
    state.set_focus(0, 0, false);
    state.set_manual_scroll_row(50);
    state.ensure_focus_visible(5);
    assert_eq!(state.scroll_row(), 50);
    state.set_focus(2, 0, false);
    state.ensure_focus_visible(5);
    assert_eq!(state.scroll_row(), 2);
}

#[test]
fn manual_horizontal_scroll_can_hide_focus() {
    let mut state = ResultsState::new();
    state.headers = (0..8).map(|idx| format!("c{idx}")).collect();
    state.rows = vec![vec!["row".into()]];
    state.set_focus(0, 0, false);
    state.set_manual_scroll_col(5);
    state.ensure_focus_col_visible(10, 1, &[2; 8]);
    assert_eq!(state.scroll_col(), 5);
    state.set_focus(0, 1, false);
    state.ensure_focus_col_visible(10, 1, &[2; 8]);
    assert_eq!(state.scroll_col(), 0);
}

#[test]
fn aliased_column_edit_uses_verified_source_name_and_physical_identity() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["display_alias".into()],
        vec![vec!["11".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "demo")),
            row_identities: vec![Some(RowIdentity {
                relation_oid: 1,
                xmin: "1".into(),
                primary_key: Vec::new(),
                table_oid: 16_384,
                ctid: "(0,1)".into(),
            })],
            source_columns: vec![Some("physical_column".into())],
            column_types: vec!["pg_catalog.int4".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert!(state.begin_edit());
    state.edit_session_mut().unwrap().insert_char('!');
    let EditCompletion::Changed(edit) = state.finish_edit().expect("edit payload") else {
        panic!("changed edit expected");
    };
    assert_eq!(edit.column_name, "physical_column");
    assert_eq!(edit.row_identity.table_oid, 16_384);
    assert_eq!(edit.row_identity.ctid, "(0,1)");
}

#[test]
fn results_without_verified_source_columns_are_read_only() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["computed".into()],
        vec![vec!["12".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "demo")),
            row_identities: vec![Some(RowIdentity {
                relation_oid: 1,
                xmin: "1".into(),
                primary_key: Vec::new(),
                table_oid: 16_384,
                ctid: "(0,1)".into(),
            })],
            source_columns: vec![None],
            column_types: vec!["pg_catalog.int4".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert!(!state.can_edit());
    assert!(!state.begin_edit());
}

#[test]
fn row_hydration_maps_physical_columns_back_to_display_aliases() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["display_id".into()],
        vec![vec!["old name".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "demo")),
            row_identities: vec![Some(RowIdentity {
                relation_oid: 1,
                xmin: "1".into(),
                primary_key: Vec::new(),
                table_oid: 16_384,
                ctid: "(0,1)".into(),
            })],
            source_columns: vec![Some("name".into())],
            column_types: vec!["pg_catalog.text".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );
    let refreshed_identity = RowIdentity {
        relation_oid: 1,
        xmin: "1".into(),
        primary_key: Vec::new(),
        table_oid: 16_384,
        ctid: "(0,2)".into(),
    };

    state.hydrate_row(
        0,
        &["id".into(), "name".into()],
        &["7".into(), "new name".into()],
        &[false, false],
        Some(refreshed_identity.clone()),
    );

    assert_eq!(state.rows[0], vec!["new name".to_string()]);
    assert_eq!(state.row_identities[0], Some(refreshed_identity));
}

#[test]
fn null_and_literal_null_remain_distinct_through_editing() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["value".into()],
        vec![vec!["NULL".into()], vec!["NULL".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "values")),
            row_identities: vec![
                Some(RowIdentity {
                    relation_oid: 1,
                    xmin: "1".into(),
                    primary_key: Vec::new(),
                    table_oid: 16_384,
                    ctid: "(0,1)".into(),
                }),
                Some(RowIdentity {
                    relation_oid: 1,
                    xmin: "1".into(),
                    primary_key: Vec::new(),
                    table_oid: 16_384,
                    ctid: "(0,2)".into(),
                }),
            ],
            source_columns: vec![Some("value".into())],
            column_types: vec!["pg_catalog.text".into()],
            null_cells: vec![vec![true], vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert!(state.begin_edit());
    assert!(state
        .edit_session()
        .is_some_and(super::EditSession::is_null));
    assert!(matches!(
        state.finish_edit(),
        Some(EditCompletion::Unchanged)
    ));

    state.set_focus(1, 0, false);
    assert!(state.begin_edit());
    assert!(state.edit_session().is_some_and(|edit| !edit.is_null()));
    assert!(state.set_edit_value_null());
    let Some(EditCompletion::Changed(edit)) = state.finish_edit() else {
        panic!("NULL change expected");
    };
    assert_eq!(edit.new_value, None);
}

#[test]
fn first_text_edit_replaces_the_sql_null_placeholder() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["value".into()],
        vec![vec!["NULL".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "values")),
            row_identities: vec![Some(RowIdentity {
                relation_oid: 1,
                xmin: "1".into(),
                primary_key: Vec::new(),
                table_oid: 16_384,
                ctid: "(0,1)".into(),
            })],
            source_columns: vec![Some("value".into())],
            column_types: vec!["pg_catalog.text".into()],
            null_cells: vec![vec![true]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert!(state.begin_edit());
    state.edit_session_mut().unwrap().insert_char('x');
    assert_eq!(state.edit_session().unwrap().buffer(), "x");
}

#[test]
fn presentation_cache_escapes_headers_without_changing_raw_data() {
    let mut state = ResultsState::new();
    let raw_header = "line\nname\u{202e}".to_string();
    state.set_data(
        vec![raw_header.clone()],
        vec![vec!["value".into()]],
        ResultMetadata {
            source_table: None,
            row_identities: vec![None],
            source_columns: vec![None],
            column_types: vec!["text".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert_eq!(state.headers[0], raw_header);
    assert_eq!(state.display_header(0), Some("line\\u{000A}name\\u{202E}"));
}

#[test]
fn presentation_cache_grows_for_cell_updates_and_row_hydration() {
    let mut state = ResultsState::new();
    state.set_data(
        vec!["name".into()],
        vec![vec!["x".into()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "demo")),
            row_identities: vec![None],
            source_columns: vec![Some("name".into())],
            column_types: vec!["text".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );
    let initial_width = state.presentation_widths()[0];

    state.apply_cell_update(0, 0, Some("a much longer value".into()));
    let updated_width = state.presentation_widths()[0];
    assert!(updated_width > initial_width);

    state.hydrate_row(
        0,
        &["name".into()],
        &["an even longer hydrated value".into()],
        &[false],
        None,
    );
    assert!(state.presentation_widths()[0] > updated_width);
}

#[test]
fn editing_keeps_the_complete_value_behind_bounded_presentation() {
    let mut state = ResultsState::new();
    let raw_value = "a".repeat(super::presentation::MAX_DISPLAY_FIELD_BYTES * 16);
    state.set_data(
        vec!["value".into()],
        vec![vec![raw_value.clone()]],
        ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "values")),
            row_identities: vec![Some(RowIdentity {
                relation_oid: 1,
                xmin: "1".into(),
                primary_key: Vec::new(),
                table_oid: 16_384,
                ctid: "(0,1)".into(),
            })],
            source_columns: vec![Some("value".into())],
            column_types: vec!["pg_catalog.text".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    assert_eq!(state.current_value(), Some(raw_value.as_str()));
    assert_eq!(
        state.presentation_widths()[0],
        super::presentation::MAX_DISPLAY_COLUMN_WIDTH
    );
    assert!(state.begin_edit());
    state.edit_session_mut().unwrap().insert_char('z');
    let Some(EditCompletion::Changed(edit)) = state.finish_edit() else {
        panic!("changed edit expected");
    };
    assert_eq!(edit.new_value, Some(format!("{raw_value}z")));
}
