use super::identifiers::{extract_identifier_before_dot, trailing_literal_punctuation};
use super::scan::SqlScan;
use super::scope::QueryScope;
use super::token::TokenWindow;
use crate::CompletionContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlClause {
    Start,
    With,
    SelectList,
    From,
    Join,
    Where,
    GroupBy,
    Having,
    Window,
    OrderBy,
    Limit,
    Fetch,
    Offset,
    Insert,
    InsertValues,
    Update,
    UpdateSet,
    Delete,
    Returning,
}

pub(crate) fn detect_context(
    buffer: &str,
    cursor: usize,
    token: &TokenWindow,
    scan: &SqlScan,
    scope: &QueryScope,
) -> CompletionContext {
    if scan.suppressed {
        return CompletionContext::Literal;
    }

    if literal_after_operator(buffer, token.start_offset) {
        return CompletionContext::Literal;
    }

    let clause = clause_from_tokens(&scan.tokens);
    let previous_non_whitespace = previous_non_whitespace(buffer, token.start_offset);

    if matches!(token.prev_char, Some(ch) if trailing_literal_punctuation(ch)) {
        return CompletionContext::Literal;
    }

    if let Some(prev_char) = previous_non_whitespace {
        if matches!(prev_char, '=' | '<' | '>' | '!') {
            return CompletionContext::Literal;
        }
    }

    if matches!(token.prev_char, Some('.')) {
        // Treat identifiers ending with a dot as schema/table hints before resolving column names.
        let ident = extract_identifier_before_dot(buffer, cursor);
        if matches!(
            clause,
            SqlClause::From
                | SqlClause::Join
                | SqlClause::Insert
                | SqlClause::Update
                | SqlClause::Delete
        ) && !(matches!(clause, SqlClause::Join) && join_condition_started(&scan.tokens))
        {
            return CompletionContext::Table { schema_hint: ident };
        }
        return CompletionContext::Column { table_hint: ident };
    }

    if tokens_end_with(&scan.tokens, &["group", "by"]) {
        return CompletionContext::GroupBy;
    }
    if tokens_end_with(&scan.tokens, &["order", "by"]) {
        return CompletionContext::OrderBy;
    }

    if let Some(word) = scan.tokens.last().map(String::as_str) {
        match word {
            "with" => return CompletionContext::Start,
            "select" | "distinct" => return CompletionContext::SelectList,
            "from" | "join"
                if previous_non_whitespace == Some('"')
                    && !scope.relations.is_empty()
                    && !buffer
                        .get(cursor..)
                        .is_some_and(|tail| tail.trim_start().starts_with('.')) => {}
            "from" | "join" | "table" | "insert" | "into" | "update" | "delete" => {
                return CompletionContext::Table { schema_hint: None };
            }
            "where" => return CompletionContext::Where,
            "having" => return CompletionContext::Having,
            "window" => return CompletionContext::Window,
            "order" | "group" => return CompletionContext::Keyword,
            "limit" | "fetch" => return CompletionContext::Limit,
            "offset" => return CompletionContext::Offset,
            "values" => return CompletionContext::InsertValues,
            "set" => return CompletionContext::UpdateSet,
            "returning" => return CompletionContext::Returning,
            "on" | "using" => return CompletionContext::JoinCondition,
            _ => {}
        }
    }

    if is_join_prefix(&scan.tokens) {
        return CompletionContext::JoinType;
    }

    match clause {
        SqlClause::Start | SqlClause::With => CompletionContext::Start,
        SqlClause::SelectList => CompletionContext::SelectList,
        SqlClause::From
        | SqlClause::Join
        | SqlClause::Insert
        | SqlClause::Update
        | SqlClause::Delete => CompletionContext::FromSource,
        SqlClause::Where => CompletionContext::Where,
        SqlClause::GroupBy => CompletionContext::GroupBy,
        SqlClause::Having => CompletionContext::Having,
        SqlClause::Window => CompletionContext::Window,
        SqlClause::OrderBy => CompletionContext::OrderBy,
        SqlClause::Limit | SqlClause::Fetch => CompletionContext::Limit,
        SqlClause::Offset => CompletionContext::Offset,
        SqlClause::InsertValues => CompletionContext::InsertValues,
        SqlClause::UpdateSet => CompletionContext::UpdateSet,
        SqlClause::Returning => CompletionContext::Returning,
    }
}

fn join_condition_started(tokens: &[String]) -> bool {
    tokens
        .iter()
        .rposition(|token| token == "join")
        .is_some_and(|join_idx| {
            tokens[join_idx + 1..]
                .iter()
                .any(|token| matches!(token.as_str(), "on" | "using"))
        })
}

fn clause_from_tokens(tokens: &[String]) -> SqlClause {
    use SqlClause::{
        Delete, Fetch, From, GroupBy, Having, Insert, InsertValues, Join, Limit, Offset, OrderBy,
        Returning, SelectList, Start, Update, UpdateSet, Where, Window, With,
    };
    let mut clause = Start;
    let mut expect_group_by = false;
    let mut expect_order_by = false;

    for word in tokens {
        if expect_group_by {
            expect_group_by = false;
            if word == "by" {
                clause = GroupBy;
                continue;
            }
        }
        if expect_order_by {
            expect_order_by = false;
            if word == "by" {
                clause = OrderBy;
                continue;
            }
        }
        match word.as_str() {
            "with" => clause = With,
            "select" => clause = SelectList,
            "from" => clause = From,
            "join" => clause = Join,
            "where" => clause = Where,
            "group" => {
                expect_group_by = true;
            }
            "having" => clause = Having,
            "window" => clause = Window,
            "order" => {
                expect_order_by = true;
            }
            "limit" => clause = Limit,
            "fetch" => clause = Fetch,
            "offset" => clause = Offset,
            "insert" => clause = Insert,
            "into" => {
                if matches!(clause, Start | Insert) {
                    clause = Insert;
                }
            }
            "values" => {
                if matches!(clause, Insert | InsertValues) {
                    clause = InsertValues;
                }
            }
            "update" => clause = Update,
            "set" => {
                if matches!(clause, Update | UpdateSet) {
                    clause = UpdateSet;
                }
            }
            "delete" => clause = Delete,
            "returning" => clause = Returning,
            "union" | "intersect" | "except" => clause = Start,
            _ => {}
        }
    }

    clause
}

// Matches a suffix of the scanned tokens without reallocating so we can detect multi-word clauses.
fn tokens_end_with(tokens: &[String], pattern: &[&str]) -> bool {
    if pattern.is_empty() || tokens.len() < pattern.len() {
        return false;
    }
    tokens
        .iter()
        .rev()
        .zip(pattern.iter().rev())
        .take(pattern.len())
        .all(|(token, expected)| token == expected)
}

fn word_at(tokens: &[String], offset_from_end: usize) -> Option<&str> {
    tokens
        .len()
        .checked_sub(offset_from_end + 1)
        .and_then(|idx| tokens.get(idx))
        .map(String::as_str)
}

// Detects partial JOIN phrases like "LEFT" or "FULL OUTER" so we can suggest join types.
fn is_join_prefix(tokens: &[String]) -> bool {
    if let Some(last) = tokens.last() {
        match last.as_str() {
            "left" | "right" | "full" | "inner" | "cross" | "natural" => return true,
            "outer" => {
                if let Some(prev) = word_at(tokens, 1) {
                    return matches!(prev, "left" | "right" | "full");
                }
            }
            _ => {}
        }
    }
    false
}

/// Returns the previous meaningful character before `limit`, skipping whitespace.
pub(crate) fn previous_non_whitespace(buffer: &str, limit: usize) -> Option<char> {
    buffer
        .get(..limit)
        .unwrap_or("")
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

/// Detects when the cursor follows a literal that itself followed an operator (e.g. `= 1`).
pub(crate) fn literal_after_operator(buffer: &str, limit: usize) -> bool {
    let slice = buffer.get(..limit).unwrap_or("");
    let mut chars = slice.chars().rev().peekable();
    let mut in_string = false;
    let mut saw_literal = false;

    while let Some(ch) = chars.next() {
        if in_string {
            if ch == '\'' {
                if matches!(chars.peek(), Some('\'')) {
                    chars.next();
                } else {
                    in_string = false;
                }
            }
            continue;
        }
        if ch.is_whitespace() {
            if !saw_literal {
                continue;
            }
            continue;
        }
        if ch == '\'' {
            in_string = true;
            saw_literal = true;
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && saw_literal) {
            saw_literal = true;
            continue;
        }
        if saw_literal {
            return matches!(ch, '=' | '<' | '>' | '!' | '+' | '-' | '*' | '/' | '%');
        }
        break;
    }

    false
}
