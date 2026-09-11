use super::fixtures::completion_service;
use crate::CompletionKind;

#[test]
fn start_context_surfaces_global_keywords() {
    // Empty buffers should still get statement starters.
    let service = completion_service();
    let buffer = "";
    let batch = service.suggest(buffer, buffer.len());
    let labels: Vec<&str> = batch.items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("SELECT")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("INSERT INTO")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("DROP TABLE")));
    assert!(labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("CREATE TABLE")));
}

#[test]
fn ddl_keywords_are_labeled() {
    let service = completion_service();
    let buffer = "";
    let batch = service.suggest(buffer, buffer.len());
    let ddl = batch
        .items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("DROP TABLE"));
    assert!(ddl.is_some(), "expected DROP TABLE in start completions");
    let ddl = ddl.unwrap();
    assert_eq!(ddl.detail, "ddl");
    assert_eq!(ddl.kind, CompletionKind::Keyword);
}

#[test]
fn ddl_completion_inserts_full_phrase_when_first_word_is_partial() {
    let service = completion_service();
    let buffer = "cre";
    let batch = service.suggest(buffer, buffer.len());
    let ddl = batch
        .items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("CREATE TABLE"))
        .expect("CREATE TABLE completion missing");
    assert_eq!(
        ddl.insert_text, "CREATE TABLE",
        "should insert the full keyword when the leading word is incomplete"
    );
}

#[test]
fn ddl_completion_only_replaces_tail_after_first_word() {
    let service = completion_service();
    let buffer = "CREATE ";
    let batch = service.suggest(buffer, buffer.len());
    let ddl = batch
        .items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("CREATE TABLE"))
        .expect("CREATE TABLE completion missing");
    assert_eq!(
        ddl.insert_text, "TABLE",
        "should only insert the trailing keyword once CREATE is already typed"
    );
}

#[test]
fn ddl_completion_tail_logic_applies_to_other_keywords() {
    let service = completion_service();
    let buffer = "DROP TA";
    let batch = service.suggest(buffer, buffer.len());
    let ddl = batch
        .items
        .iter()
        .find(|item| item.label.eq_ignore_ascii_case("DROP TABLE"))
        .expect("DROP TABLE completion missing");
    assert_eq!(
        ddl.insert_text, "TABLE",
        "DROP TABLE should also only insert TABLE when DROP is already present"
    );
}
