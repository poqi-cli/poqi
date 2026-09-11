use super::fixtures::{completion_service, multi_schema_service};
use crate::CompletionKind;

#[test]
fn literal_context_suppresses_suggestions() {
    // Once an operator is present, literal contexts suppress completion entirely.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE id = ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());

    let buffer_with_value = "SELECT * FROM public.widgets WHERE id = 42";
    let batch = service.suggest(buffer_with_value, buffer_with_value.len());
    assert!(batch.items.is_empty());
}

#[test]
fn keyword_context_after_where() {
    // WHERE context should still offer logical keywords.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("AND")));
}

#[test]
fn literal_context_hides_after_operator() {
    // Edge regression: cursor directly after operator still suppresses options.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE id = ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn literal_context_hides_after_numeric_with_space() {
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE id = 1 ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn literal_context_hides_after_string_with_space() {
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE name = 'alpha' ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn literal_context_hides_after_punctuation() {
    let service = completion_service();
    for punct in [":", ",", "-"] {
        let buffer = format!("SELECT * FROM public.widgets WHERE id = 1{punct}");
        let batch = service.suggest(&buffer, buffer.len());
        assert!(
            batch.items.is_empty(),
            "punctuation {punct} should suppress completions"
        );
    }
}

#[test]
fn where_context_surfaces_columns() {
    // WHERE clauses should always highlight matching columns.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("id")));
}

#[test]
fn predicate_snippet_offers_equals_template() {
    // Predicate snippets bubble up alongside columns to speed WHERE authoring.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.kind == CompletionKind::Snippet && item.label == "id = ?"));
}

#[test]
fn where_scopes_columns_to_primary_table() {
    // WHERE should prefer the referenced table's columns, not unrelated tables.
    let service = multi_schema_service();
    let buffer = "SELECT * FROM public.widgets WHERE ";
    let batch = service.suggest(buffer, buffer.len());
    let labels: Vec<&str> = batch.items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"id"), "expected widgets.id to appear");
    assert!(
        !labels.contains(&"code"),
        "unrelated columns must not appear in WHERE suggestions"
    );
}

#[test]
fn join_type_suggests_expected_tokens() {
    // Partial JOIN phrases should expand into the expected join keywords.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets LEFT ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("LEFT JOIN")));
}

#[test]
fn comments_suppress_suggestions() {
    // Typing inside line comments must not trigger completion popups.
    let service = completion_service();
    let buffer = "SELECT -- typing in comment";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn order_by_context_includes_modifiers() {
    // ORDER BY completions include ASC/DESC modifiers.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets ORDER BY ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("ASC")));
}

#[test]
fn limit_context_suppresses_clause_tokens() {
    // Numeric clauses should never offer new SQL keywords.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets LIMIT ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn offset_context_suppresses_clause_tokens() {
    // Offset values behave like LIMIT—no clause keywords allowed.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets OFFSET ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());
}

#[test]
fn semicolon_terminator_hides_suggestions() {
    // Semicolons terminate the statement and should suppress further results.
    let service = completion_service();
    let buffer = "SELECT * FROM public.widgets WHERE id = 1;";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.is_empty());

    let buffer_with_space = "SELECT * FROM public.widgets WHERE id = 1;   ";
    let batch = service.suggest(buffer_with_space, buffer_with_space.len());
    assert!(batch.items.is_empty());
}
