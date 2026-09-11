use crate::{match_score, CompletionItem, CompletionKind};

/// Small helpers for building common completion rows inside start-keyword handlers.
pub(crate) fn keyword_item(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        insert_text: label.to_string(),
        detail: detail.to_string(),
        kind: CompletionKind::Keyword,
        score: 0,
    }
}

pub(crate) fn keyword_item_with_insert(
    label: &str,
    insert_text: &str,
    detail: &str,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        insert_text: insert_text.to_string(),
        detail: detail.to_string(),
        kind: CompletionKind::Keyword,
        score: 0,
    }
}

pub(crate) fn push_keyword_match(
    prefix: &str,
    label: &str,
    detail: &str,
    candidates: &mut Vec<CompletionItem>,
) {
    if match_score(prefix, label).is_some() {
        candidates.push(keyword_item(label, detail));
    }
}

/// Adds a comma completion row when the prefix matches. Returns true when a row was added.
pub(crate) fn push_comma_candidate(prefix: &str, candidates: &mut Vec<CompletionItem>) -> bool {
    if match_score(prefix, ",").is_none() {
        return false;
    }
    candidates.push(keyword_item_with_insert(",", ", ", "punctuation"));
    true
}
