use poqi_catalog::{
    CatalogSnapshot, ColumnDefault, ColumnMeta, Nullability, RelationKind, TableId, TableMeta,
};

use super::fixtures::{completion_service, multi_schema_service};
use crate::{analyzer::analyze_query_scope, CompletionKind, CompletionService};

#[test]
fn aliases_scope_columns_in_where_select_and_join_on() {
    let service = multi_schema_service();
    for (sql, cursor) in [
        (
            "SELECT * FROM public.widgets w WHERE w.",
            "SELECT * FROM public.widgets w WHERE w.".len(),
        ),
        ("SELECT w. FROM public.widgets AS w", "SELECT w.".len()),
    ] {
        let scope = analyze_query_scope(sql, cursor);
        assert!(
            scope.has_qualifier("w"),
            "alias missing from scope: {scope:#?}"
        );
        let labels = service
            .suggest(sql, cursor)
            .items
            .iter()
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();
        assert!(
            labels.iter().any(|label| label == "id"),
            "widgets alias should resolve in {sql}; got {labels:?}"
        );
        assert!(
            !labels.iter().any(|label| label == "code"),
            "unrelated columns leaked in {sql}"
        );
    }

    let join_sql = "SELECT * FROM public.widgets w JOIN analytics.gadgets g ON g.";
    let labels = labels(&service, join_sql);
    assert!(labels.iter().any(|label| label == "code"));
    assert!(!labels.iter().any(|label| label == "id"));
}

#[test]
fn unknown_explicit_qualifier_fails_closed() {
    let service = multi_schema_service();
    let sql = "SELECT * FROM public.widgets w WHERE missing.";
    let batch = service.suggest(sql, sql.len());

    assert!(
        batch
            .items
            .iter()
            .all(|item| item.kind != CompletionKind::Column),
        "unknown qualifiers must not fall back to every catalog column"
    );
}

#[test]
fn unqualified_columns_are_limited_to_current_statement_sources() {
    let service = multi_schema_service();
    let sql = "SELECT * FROM analytics.gadgets; SELECT  FROM public.widgets WHERE ";
    let labels = labels(&service, sql);

    assert!(labels.iter().any(|label| label == "id"));
    assert!(!labels.iter().any(|label| label == "code"));
}

#[test]
fn quoted_alias_and_relation_components_resolve_losslessly() {
    let service = CompletionService::new(quoted_snapshot());
    let sql = "SELECT \"o.i\". FROM \"Odd.Schema\".\"Order.Items\" AS \"o.i\"";
    let batch = service.suggest(sql, "SELECT \"o.i\".".len());
    let column = batch
        .items
        .iter()
        .find(|item| item.kind == CompletionKind::Column)
        .expect("quoted alias column");

    assert_eq!(column.label, "Line\"Total");
    assert_eq!(column.insert_text, "\"Line\"\"Total\"");
}

#[test]
fn catalog_insertions_quote_each_identifier_component() {
    let service = CompletionService::new(quoted_snapshot());

    let from_sql = "SELECT * FROM Ord";
    let table = service
        .suggest(from_sql, from_sql.len())
        .items
        .into_iter()
        .find(|item| item.kind == CompletionKind::Table)
        .expect("quoted table completion");
    assert_eq!(table.label, "Odd.Schema.Order.Items");
    assert_eq!(table.insert_text, "\"Odd.Schema\".\"Order.Items\"");

    let column_sql = "SELECT  FROM \"Odd.Schema\".\"Order.Items\"";
    let column = service
        .suggest(column_sql, "SELECT ".len())
        .items
        .into_iter()
        .find(|item| item.kind == CompletionKind::Column)
        .expect("quoted column completion");
    assert_eq!(column.insert_text, "\"Line\"\"Total\"");
}

#[test]
fn completed_quoted_from_relation_transitions_to_clause_suggestions() {
    let service = completion_service();

    for sql in [
        "SELECT * FROM \"public\".\"widgets\"",
        "SELECT * FROM \"public\".\"widgets\" ",
    ] {
        let batch = service.suggest(sql, sql.len());

        assert!(
            batch.items.iter().any(|item| item.label == "WHERE"),
            "completed quoted relation should offer clauses for {sql}; got {:#?}",
            batch.items
        );
        assert!(
            batch
                .items
                .iter()
                .all(|item| item.kind != CompletionKind::Table),
            "completed quoted relation should not repeat table/schema suggestions for {sql}"
        );
    }
}

#[test]
fn completed_quoted_schema_dot_still_offers_tables() {
    let service = completion_service();
    let sql = "SELECT * FROM \"public\".";
    let batch = service.suggest(sql, sql.len());

    assert!(batch
        .items
        .iter()
        .any(|item| item.kind == CompletionKind::Table && item.label == "widgets"));
}

#[test]
fn caret_before_schema_dot_does_not_treat_schema_as_a_completed_relation() {
    let service = completion_service();
    let prefix = "SELECT * FROM \"public\"";

    for tail in [".\"widgets\"", " . \"widgets\""] {
        let sql = format!("{prefix}{tail}");
        let batch = service.suggest(&sql, prefix.len());

        assert!(batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Table));
        assert!(batch.items.iter().all(|item| item.label != "WHERE"));
    }
}

#[test]
fn duplicate_from_candidates_keep_only_the_best_scored_item() {
    let service = completion_service();
    let sql = "SELECT id ";
    let batch = service.suggest(sql, sql.len());

    assert_eq!(
        batch
            .items
            .iter()
            .filter(|item| item.label.eq_ignore_ascii_case("FROM"))
            .count(),
        1
    );
}

#[test]
fn select_star_alias_does_not_reoffer_star_columns_or_functions() {
    let service = completion_service();
    let sql = "SELECT * AS selected ";
    let batch = service.suggest(sql, sql.len());

    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("FROM")));
    assert!(batch.items.iter().all(|item| {
        item.kind != CompletionKind::Column
            && item.detail != "aggregate"
            && item.label.as_str() != "*"
    }));
}

#[test]
fn arithmetic_star_does_not_suppress_select_expression_completions() {
    let service = completion_service();
    let sql = "SELECT price *  FROM public.widgets";
    let cursor = "SELECT price * ".len();
    let batch = service.suggest(sql, cursor);

    assert!(batch.items.iter().any(|item| item.label == "id"));
    assert!(batch.items.iter().any(|item| item.detail == "aggregate"));
}

fn labels(service: &CompletionService, sql: &str) -> Vec<String> {
    service
        .suggest(sql, sql.len())
        .items
        .iter()
        .map(|item| item.label.clone())
        .collect()
}

fn quoted_snapshot() -> CatalogSnapshot {
    CatalogSnapshot {
        tables: vec![TableMeta {
            id: TableId("quoted-order-items".to_string()),
            name: "Order.Items".to_string(),
            schema: "Odd.Schema".to_string(),
            relation_kind: RelationKind::Table,
        }],
        columns: vec![ColumnMeta {
            schema: "Odd.Schema".to_string(),
            table: "Order.Items".to_string(),
            name: "Line\"Total".to_string(),
            data_type: "numeric".to_string(),
            ordinal_position: 1,
            nullability: Nullability::Nullable,
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            is_primary_key: false,
            default_kind: ColumnDefault::None,
            is_foreign_key: false,
        }],
        foreign_keys: Vec::new(),
    }
}
