use std::path::Path;

use super::*;
use poqi_catalog::QualifiedRelation;
use poqi_engine::{ResultMetadata, RowIdentity};

fn sample_result() -> QueryResult {
    QueryResult {
        columns: vec!["id".into(), "name".into()],
        rows: vec![
            vec!["1".into(), "alpha".into()],
            vec!["2".into(), "beta".into()],
        ],
        metadata: ResultMetadata {
            source_table: Some(QualifiedRelation::in_schema("public", "demo")),
            row_identities: vec![
                Some(RowIdentity {
                    relation_oid: 1,
                    xmin: "1".into(),
                    primary_key: Vec::new(),
                    table_oid: 1,
                    ctid: "(0,1)".into(),
                }),
                Some(RowIdentity {
                    relation_oid: 1,
                    xmin: "1".into(),
                    primary_key: Vec::new(),
                    table_oid: 1,
                    ctid: "(0,2)".into(),
                }),
            ],
            source_columns: vec![Some("id".into()), Some("name".into())],
            column_types: Vec::new(),
            null_cells: vec![vec![false, false], vec![false, false]],
            origin: ResultOrigin::SelectTop {
                table: QualifiedRelation::in_schema("public", "demo"),
                limit: 2,
            },
        },
    }
}

#[test]
fn row_keys_follow_content_even_when_ctids_are_present() {
    let result = sample_result();
    let initial = row_keys(&result);
    let mut changed = result;
    changed.rows[0][1] = "gamma".to_string();
    changed.metadata.row_identities[0] = Some(RowIdentity {
        relation_oid: 1,
        xmin: "1".into(),
        primary_key: Vec::new(),
        table_oid: 2,
        ctid: "(0,1)".to_string(),
    });
    let updated = row_keys(&changed);
    assert_ne!(initial[0], updated[0]);
    assert_eq!(initial[1], updated[1]);
}

#[test]
fn row_keys_do_not_depend_on_ctids() {
    let mut result = sample_result();
    let with_ctids = row_keys(&result);
    result.metadata.row_identities = vec![None, None];
    let without_ctids = row_keys(&result);
    assert_eq!(with_ctids, without_ctids);
    assert_ne!(
        hash_row(&result.rows[0], &result.columns, None),
        hash_row(&result.rows[1], &result.columns, None)
    );
}

#[test]
fn row_keys_distinguish_sql_null_from_literal_null_text() {
    let mut result = sample_result();
    result.rows[0][1] = "NULL".to_string();
    result.rows[1][1] = "NULL".to_string();
    result.metadata.null_cells = vec![vec![false, true], vec![false, false]];

    let keys = row_keys(&result);
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn dataset_id_remains_stable_for_incremental_row_refresh() {
    let mut result = sample_result();
    result.metadata.row_identities = vec![None, None];
    let first = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("name"));

    result.rows[0][1] = "gamma".to_string();
    let second = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("name"));
    assert_eq!(first, second);
}

#[test]
fn dataset_id_changes_when_limit_differs() {
    let mut result = sample_result();
    let first = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("name"));
    result.metadata.origin = ResultOrigin::SelectTop {
        table: QualifiedRelation::in_schema("public", "demo"),
        limit: 1,
    };
    let second = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("name"));
    assert_ne!(first, second);
}

#[test]
fn dataset_id_changes_when_title_column_differs() {
    let result = sample_result();
    let first = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("name"));
    let second = build_dataset_id(&result, Path::new("/tmp/model.onnx"), Some("id"));
    assert_ne!(first, second);
}
