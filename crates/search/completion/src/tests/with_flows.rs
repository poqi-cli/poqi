use super::fixtures::completion_service;

#[test]
fn with_column_list_suggests_closing_paren() {
    let service = completion_service();
    let buffer = "WITH cte_name (";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch.items.iter().any(|item| item.label == ")"),
        "column list should offer closing paren"
    );
}

#[test]
fn with_not_materialized_prompts_keyword() {
    let service = completion_service();
    let buffer = "WITH cte_name AS NOT ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("MATERIALIZED")),
        "expect MATERIALIZED suggestion after AS NOT"
    );
    assert!(
        !batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("NOT MATERIALIZED")),
        "should not offer NOT MATERIALIZED once NOT is already present"
    );
}

#[test]
fn with_select_body_uses_select_completions() {
    let service = completion_service();
    let buffer = "WITH cte_name AS (SELECT id ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("FROM")),
        "SELECT body should surface FROM keyword"
    );
}

#[test]
fn with_insert_body_uses_insert_completions() {
    let service = completion_service();
    let buffer = "WITH cte_name AS (INSERT INTO public.widgets (id) ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("VALUES")),
        "INSERT body should surface VALUES keyword"
    );
}

#[test]
fn with_after_cte_surfaces_statement_keywords() {
    let service = completion_service();
    let buffer = "WITH cte_name AS (SELECT 1) ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("SELECT")),
        "after WITH clause we should show statement keywords"
    );
}

#[test]
fn with_literal_suggests_closing_paren() {
    let service = completion_service();
    let buffer = "WITH cte_name AS (SELECT * FROM public.widgets WHERE id = 1 ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch.items.iter().any(|item| item.label == ")"),
        "literal context should offer closing paren"
    );
    assert!(
        !batch.items.iter().any(|item| item.label == "),"),
        "literal context should not offer joint paren+comma snippet"
    );
}

#[test]
fn with_after_cte_select_uses_select_logic() {
    let service = completion_service();
    let buffer = "WITH cte_name AS (SELECT 1) SELECT id ";
    let batch = service.suggest(buffer, buffer.len());
    assert!(
        batch
            .items
            .iter()
            .any(|item| item.label.eq_ignore_ascii_case("FROM")),
        "SELECT following a CTE should get SELECT completions"
    );
}
