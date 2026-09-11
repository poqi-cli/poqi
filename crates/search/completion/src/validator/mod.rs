use pg_query::parse;

use crate::{CompletionItem, CompletionKind};

/// Avoids parsing every candidate while still catching obviously wrong insertions.
const MAX_VALIDATED_CANDIDATES: usize = 16;

/// Uses `pg_query` to sanity-check a small slice of completion insertions.
#[derive(Debug)]
pub(crate) struct PgQueryValidator {
    max_candidate_checks: usize,
}

impl Default for PgQueryValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl PgQueryValidator {
    pub(crate) fn new() -> Self {
        Self {
            max_candidate_checks: MAX_VALIDATED_CANDIDATES,
        }
    }

    pub(crate) fn filter(
        &self,
        buffer: &str,
        cursor: usize,
        token_start: usize,
        candidates: Vec<CompletionItem>,
    ) -> Vec<CompletionItem> {
        if buffer.trim().is_empty() || !has_validated_candidate(&candidates) {
            return candidates;
        }

        if parse(buffer).is_err() {
            return candidates;
        }

        let cursor = cursor.min(buffer.len());
        let token_start = token_start.min(cursor);
        let head = &buffer[..token_start];
        let tail = &buffer[cursor..];

        let mut validated = 0usize;
        candidates
            .into_iter()
            .filter(|item| {
                if let Some(kind) = classify_candidate(item) {
                    if validated >= self.max_candidate_checks {
                        return true;
                    }
                    validated += 1;
                    return parses_with_candidate(head, tail, item, kind);
                }
                true
            })
            .collect()
    }
}

fn has_validated_candidate(items: &[CompletionItem]) -> bool {
    items.iter().any(|item| classify_candidate(item).is_some())
}

fn classify_candidate(item: &CompletionItem) -> Option<InsertionKind> {
    if let Some(keyword) = classify_clause_keyword(item) {
        return Some(InsertionKind::Clause(keyword));
    }
    if item.kind == CompletionKind::Snippet {
        return Some(InsertionKind::PredicateEquals);
    }
    None
}

fn classify_clause_keyword(item: &CompletionItem) -> Option<ClauseKeyword> {
    if item.kind != CompletionKind::Keyword {
        return None;
    }
    match item.insert_text.trim().to_ascii_uppercase().as_str() {
        "WHERE" => Some(ClauseKeyword::Where),
        "JOIN" => Some(ClauseKeyword::Join),
        "GROUP BY" => Some(ClauseKeyword::GroupBy),
        "ORDER BY" => Some(ClauseKeyword::OrderBy),
        "LIMIT" => Some(ClauseKeyword::Limit),
        "OFFSET" => Some(ClauseKeyword::Offset),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum ClauseKeyword {
    Where,
    Join,
    GroupBy,
    OrderBy,
    Limit,
    Offset,
}

#[derive(Clone, Copy)]
enum InsertionKind {
    Clause(ClauseKeyword),
    /// `col =` snippets that require a value placeholder to stay parseable.
    PredicateEquals,
}

fn parses_with_candidate(
    head: &str,
    tail: &str,
    item: &CompletionItem,
    kind: InsertionKind,
) -> bool {
    let placeholder = placeholder(kind);
    let mut stmt =
        String::with_capacity(head.len() + item.insert_text.len() + placeholder.len() + tail.len());
    stmt.push_str(head);
    stmt.push_str(&item.insert_text);
    stmt.push_str(placeholder);
    stmt.push_str(tail);
    parse(&stmt).is_ok()
}

/// Minimal SQL fragments that keep `pg_query` happy for each insertion class.
fn placeholder(kind: InsertionKind) -> &'static str {
    match kind {
        InsertionKind::Clause(keyword) => match keyword {
            ClauseKeyword::Where => " true",
            ClauseKeyword::Join => " __poqi_join ON true",
            ClauseKeyword::GroupBy
            | ClauseKeyword::OrderBy
            | ClauseKeyword::Limit
            | ClauseKeyword::Offset => " 1",
        },
        InsertionKind::PredicateEquals => "1",
    }
}

#[cfg(test)]
mod tests;
