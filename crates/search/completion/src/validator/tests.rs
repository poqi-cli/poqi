use super::*;

fn clause_item(label: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        insert_text: label.to_string(),
        detail: "clause".to_string(),
        kind: CompletionKind::Keyword,
        score: 0,
    }
}

fn validator() -> PgQueryValidator {
    PgQueryValidator::new()
}

#[test]
fn drops_clause_that_breaks_valid_query() {
    // WHERE after LIMIT must be rejected because it corrupts the finished query.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets LIMIT 5 ";
    let cursor = buffer.len();
    let items = vec![clause_item("WHERE"), clause_item("GROUP BY")];

    let filtered = validator.filter(buffer, cursor, cursor, items);
    assert!(filtered.is_empty());
}

#[test]
fn keeps_clause_when_query_does_not_parse() {
    // If the buffer fails to parse we skip validation altogether.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets LIMIT ";
    let cursor = buffer.len();
    let items = vec![clause_item("WHERE")];

    let filtered = validator.filter(buffer, cursor, cursor, items);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "WHERE");
}

#[test]
fn clause_remains_when_position_is_valid() {
    // Valid insertion points should keep their clause suggestions.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets ";
    let cursor = buffer.len();
    let items = vec![clause_item("WHERE")];

    let filtered = validator.filter(buffer, cursor, cursor, items);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "WHERE");
}

#[test]
fn non_clause_candidates_bypass_validation() {
    // Only risky clause/snippet insertions go through pg_query.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets LIMIT 5 ";
    let cursor = buffer.len();
    let items = vec![
        clause_item("WHERE"),
        CompletionItem {
            label: "public.widgets".to_string(),
            insert_text: "public.widgets".to_string(),
            detail: "table".to_string(),
            kind: CompletionKind::Table,
            score: 1,
        },
    ];

    let filtered = validator.filter(buffer, cursor, cursor, items);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "public.widgets");
}

#[test]
fn partially_typed_clause_keeps_suggestions() {
    // Already-typed prefixes shouldn't be punished while finishing the token.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets W";
    let cursor = buffer.len();
    let token_start = cursor - 1;
    let items = vec![clause_item("WHERE")];

    let filtered = validator.filter(buffer, cursor, token_start, items);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "WHERE");
}

#[test]
fn predicate_snippet_survives_validation() {
    // Predicate snippets are padded with values, so they remain valid.
    let validator = validator();
    let buffer = "SELECT * FROM public.widgets WHERE ";
    let cursor = buffer.len();
    let items = vec![CompletionItem {
        label: "id = ?".to_string(),
        insert_text: "id = ".to_string(),
        detail: "predicate".to_string(),
        kind: CompletionKind::Snippet,
        score: 0,
    }];

    let filtered = validator.filter(buffer, cursor, cursor, items);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "id = ?");
}
