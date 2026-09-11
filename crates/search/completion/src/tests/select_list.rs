use super::fixtures::completion_service;

#[test]
fn select_list_prioritizes_star() {
    let service = completion_service();
    let buffer = "SELECT ";
    let batch = service.suggest(buffer, buffer.len());
    let first = batch.items.first().map(|item| item.label.as_str());
    assert_eq!(
        first,
        Some("*"),
        "expected '*' to lead select list suggestions"
    );
}

#[test]
fn select_list_suppresses_second_star() {
    let service = completion_service();
    let buffer = "SELECT * ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(!batch.items.iter().any(|item| item.label == "*"));
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("FROM")));
}

#[test]
fn select_list_prefers_from_after_columns() {
    let service = completion_service();
    let buffer = "SELECT id ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(batch
        .items
        .iter()
        .any(|item| item.label.eq_ignore_ascii_case("FROM")));
}
