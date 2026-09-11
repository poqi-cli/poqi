use super::fixtures::completion_service;
use crate::CompletionKind;

#[test]
fn alter_table_surfaces_table_actions() {
    let service = completion_service();
    let buffer = "ALTER TABLE public.widgets ";
    let batch = service.suggest(buffer, buffer.len());
    let labels: Vec<&str> = batch.items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("ADD COLUMN")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("DROP COLUMN")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("RENAME COLUMN")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("RENAME TO")));
}

#[test]
fn alter_table_drop_column_lists_table_columns() {
    let service = completion_service();
    let buffer = "ALTER TABLE public.widgets DROP COLUMN ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == "id"));

    // Fallback still returns something even if the table hint is missing.
    let buffer_without_hint = "ALTER TABLE DROP COLUMN ";
    let batch = service.suggest(buffer_without_hint, buffer_without_hint.len());
    assert!(!batch.items.is_empty());
}

#[test]
fn alter_table_rename_column_lists_columns_before_name() {
    let service = completion_service();
    let buffer = "ALTER TABLE public.widgets RENAME COLUMN ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == "id"));
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("TO")),
        "TO should wait until the old column is specified"
    );
}

#[test]
fn alter_table_rename_column_prompts_to() {
    let service = completion_service();
    let buffer = "ALTER TABLE public.widgets RENAME COLUMN id ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("TO")));
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Column),
        "column suggestions should stop once the old column is named"
    );
}

#[test]
fn create_table_offers_paren_after_name() {
    let service = completion_service();
    let buffer = "CREATE TABLE widgets ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == "("));

    let buffer_with_extra_tokens = "CREATE TABLE widgets id ";
    let batch = service.suggest(buffer_with_extra_tokens, buffer_with_extra_tokens.len());
    assert!(
        !batch.items.iter().any(|item| item.label == "("),
        "opening paren should only appear immediately after the table name"
    );
}

#[test]
fn create_table_suggests_closing_paren_after_columns() {
    let service = completion_service();
    let buffer = "CREATE TABLE public.widgets (id ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == ");"));
}

#[test]
fn create_table_suggests_open_paren_after_name_only() {
    let service = completion_service();
    let buffer = "CREATE TABLE ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == "("));

    let buffer_with_name = "CREATE TABLE widgets ";
    let batch = service.suggest(buffer_with_name, buffer_with_name.len());
    assert!(
        batch.items.iter().any(|item| item.label == "("),
        "should offer '(' immediately after CREATE TABLE even before a name"
    );
}
