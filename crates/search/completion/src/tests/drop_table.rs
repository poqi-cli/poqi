use super::fixtures::completion_service;
use crate::CompletionKind;

#[test]
fn drop_table_offers_if_exists() {
    let service = completion_service();
    let buffer = "DROP TABLE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("IF EXISTS")));
}

#[test]
fn drop_table_initial_suggests_tables() {
    let service = completion_service();
    let buffer = "DROP TABLE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Table),
        "initial DROP TABLE should suggest table targets"
    );
}

#[test]
fn drop_table_after_first_target_suggests_comma_only() {
    let service = completion_service();
    let buffer = "DROP TABLE public.widgets ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch.items.iter().any(|item| item.label == ","),
        "after first target we should only prompt comma"
    );
    assert!(
        batch
            .items
            .iter()
            .all(|item| item.kind != CompletionKind::Table),
        "tables should not be suggested until a comma is typed"
    );
}

#[test]
fn drop_table_after_comma_suggests_tables_again() {
    let service = completion_service();
    let buffer = "DROP TABLE public.widgets, ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Table),
        "after comma we should suggest another table"
    );
}
