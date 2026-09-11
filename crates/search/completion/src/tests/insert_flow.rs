use super::fixtures::completion_service;
use crate::CompletionKind;

#[test]
fn insert_first_column_uses_paren_insert_text() {
    let service = completion_service();
    let buffer = "INSERT INTO public.widgets (";
    let batch = service.suggest(buffer, buffer.len());
    let column = batch
        .items
        .iter()
        .find(|item| item.kind == CompletionKind::Column && item.label == "id");
    assert_eq!(column.map(|item| item.insert_text.as_str()), Some("\"id\""));
}

#[test]
fn insert_additional_columns_use_commas() {
    let service = completion_service();
    let buffer = "INSERT INTO public.widgets (id, ";
    let batch = service.suggest(buffer, buffer.len());
    let column = batch
        .items
        .iter()
        .find(|item| item.kind == CompletionKind::Column && item.label == "id");
    assert_eq!(column.map(|item| item.insert_text.as_str()), Some("\"id\""));
}

#[test]
fn insert_values_surface_after_columns() {
    let service = completion_service();
    let buffer = "INSERT INTO public.widgets (id) ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("VALUES")));
}

#[test]
fn insert_values_offer_only_parens() {
    let service = completion_service();
    let buffer = "INSERT INTO public.widgets (id) VALUES ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == "("));
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.kind == CompletionKind::Column),
        "column names should not be suggested after VALUES"
    );
}

#[test]
fn insert_values_suggest_closing_paren() {
    let service = completion_service();
    let buffer = "INSERT INTO public.widgets (id) VALUES (1 ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch.items.iter().any(|item| item.label == ");"));
}
