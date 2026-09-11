use super::fixtures::{completion_service, deceptive_metadata_service, multi_schema_service};
use crate::CompletionKind;
use poqi_catalog::quote_identifier;

#[test]
fn table_suggestions_surface_snapshot_entries() {
    // Regression: FROM completions must surface catalog tables.
    let service = completion_service();
    let buffer = "SELECT * FROM ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label == "public.widgets"));
}

#[test]
fn column_suggestions_trigger_after_dot() {
    // Regression: dotted identifiers should flip into column mode.
    let service = completion_service();
    let buffer = "SELECT public.widgets.";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.insert_text == "\"id\""));
}

#[test]
fn column_detail_prefers_table_name_only() {
    let service = completion_service();
    let buffer = "SELECT public.widgets.";
    let batch = service.suggest(buffer, buffer.len());
    let detail = batch
        .items
        .iter()
        .find(|item| item.kind == CompletionKind::Column)
        .map(|item| item.detail.as_str());
    assert_eq!(detail, Some("widgets"));
}

#[test]
fn column_detail_includes_schema_when_needed() {
    let service = multi_schema_service();
    let buffer = "SELECT ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .filter(|item| item.kind == CompletionKind::Column)
        .any(|item| item.detail.contains('.')));
}

#[test]
fn schema_prefix_filters_tables() {
    // Typing a schema + dot restricts table suggestions to that schema.
    let service = completion_service();
    let buffer = "SELECT * FROM public.";
    let batch = service.suggest(buffer, buffer.len());
    assert_eq!(
        batch
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["widgets"]
    );
}

#[test]
fn schema_prefixed_tables_display_table_detail() {
    let service = multi_schema_service();
    let buffer = "SELECT * FROM analytics.";
    let batch = service.suggest(buffer, buffer.len());
    let detail = batch
        .items
        .iter()
        .find(|item| item.kind == CompletionKind::Table)
        .map(|item| item.detail.as_str());
    assert_eq!(detail, Some("table"));
}

#[test]
fn schema_prefix_without_clause_surfaces_tables_only() {
    // Typing a schema prefix alone should still show tables, not columns.
    let service = multi_schema_service();
    let buffer = "analytics.";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .all(|item| item.kind == CompletionKind::Table),
        "expected only table suggestions for schema prefix"
    );
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("widgets")));
}

#[test]
fn schemas_hidden_after_table_completion() {
    let service = multi_schema_service();
    let buffer = "SELECT * FROM public.widgets ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(!batch
        .items
        .iter()
        .any(|item| item.detail.eq_ignore_ascii_case("schema")));
}

#[test]
fn schemas_surface_at_from_start() {
    let service = multi_schema_service();
    let buffer = "SELECT * FROM ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.detail.eq_ignore_ascii_case("schema")));
}

#[test]
fn typing_schema_initial_keeps_suggestions() {
    let service = multi_schema_service();
    let buffer = "SELECT * FROM a";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.detail.eq_ignore_ascii_case("schema") && item.label == "analytics"));
}

#[test]
fn data_sources_hidden_until_prefix_present() {
    // Keep FROM suggestions calm until a prefix indicates a lateral/function source.
    let service = completion_service();
    let buffer = "SELECT * FROM ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(!batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("LATERAL")));

    let buffer_with_prefix = "SELECT * FROM l";
    let batch = service.suggest(buffer_with_prefix, buffer_with_prefix.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("LATERAL")));
}

#[test]
fn metadata_labels_escape_deceptive_names_without_changing_insertions() {
    let service = deceptive_metadata_service();
    let buffer = "SELECT * FROM ";
    let batch = service.suggest(buffer, buffer.len());
    let bidi_name = "audit\u{202E}cod";
    let literal_escape_name = r"audit\u{202E}cod";

    let bidi = batch
        .items
        .iter()
        .find(|item| item.insert_text.ends_with(&quote_identifier(bidi_name)))
        .expect("bidi table completion");
    let literal = batch
        .items
        .iter()
        .find(|item| {
            item.insert_text
                .ends_with(&quote_identifier(literal_escape_name))
        })
        .expect("literal escape table completion");

    assert_eq!(bidi.label, "public.audit\\u{202E}cod");
    assert_eq!(literal.label, "public.audit\\\\u{202E}cod");
    assert_ne!(bidi.label, literal.label);
    assert_eq!(
        bidi.insert_text,
        format!("\"public\".{}", quote_identifier(bidi_name))
    );
    assert_eq!(
        literal.insert_text,
        format!("\"public\".{}", quote_identifier(literal_escape_name))
    );

    let column_buffer = "SELECT ";
    let columns = service.suggest(column_buffer, column_buffer.len());
    let column = columns
        .items
        .iter()
        .find(|item| item.insert_text == quote_identifier("line\nitem"))
        .expect("control-character column completion");
    assert_eq!(column.label, "line\\u{000A}item");
    assert_eq!(column.detail, "audit\\u{202E}cod");
}
