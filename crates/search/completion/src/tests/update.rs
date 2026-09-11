use super::fixtures::completion_service;
use crate::CompletionKind;

#[test]
fn update_offers_set_after_table() {
    let service = completion_service();
    let buffer = "UPDATE public.widgets ";
    let batch = service.suggest(buffer, buffer.len());
    let first = batch.items.first().map(|item| item.label.as_str());
    assert_eq!(first, Some("SET"));
}

#[test]
fn update_offers_where_after_set() {
    let service = completion_service();
    let buffer = "UPDATE public.widgets SET id = 1 ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("WHERE")));
}

#[test]
fn update_waits_for_assignments_before_where() {
    let service = completion_service();
    let buffer = "UPDATE public.widgets SET ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("WHERE")),
        "WHERE should not surface until an assignment exists"
    );
}

#[test]
fn update_set_prefers_assignment_snippets() {
    let service = completion_service();
    let buffer = "UPDATE public.widgets SET ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Snippet && item.label == "id = ?"),
        "SET context should surface assignment snippets"
    );
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Column && item.label == "id"),
        "raw column completions should be suppressed in SET"
    );
}

#[test]
fn update_suppresses_value_keywords_after_literal() {
    let service = completion_service();
    let buffer = "UPDATE public.widgets SET id = 1 ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("WHERE")),
        "WHERE should surface after an assignment"
    );
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.detail.eq_ignore_ascii_case("literal")),
        "literal value keywords should be hidden once a value is present"
    );
}
