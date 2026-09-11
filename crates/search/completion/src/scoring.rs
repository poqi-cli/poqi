pub(crate) const TOKEN_SCORE_SCALE: u32 = 32;

/// Basic prefix-first scoring keeps ordering stable without fuzzy weight tables.
pub(crate) fn match_score(prefix: &str, candidate: &str) -> Option<u32> {
    if prefix.is_empty() {
        return Some(1);
    }
    let candidate_lower = candidate.to_ascii_lowercase();
    if prefix == "*" && candidate_lower == "*" {
        return None;
    }
    if candidate_lower.starts_with(prefix) {
        return Some(0);
    }
    if candidate_lower.contains(prefix) {
        return Some(10);
    }
    None
}

pub(crate) fn adjusted_token_score(detail: &str, token: &str, score: u32) -> u32 {
    let priority = token_priority(detail, token).min(TOKEN_SCORE_SCALE - 1);
    score
        .saturating_mul(TOKEN_SCORE_SCALE)
        .saturating_add(TOKEN_SCORE_SCALE - 1 - priority)
}

pub(crate) fn is_table_identifier(token: &str) -> bool {
    !matches!(
        token,
        "from" | "join" | "table" | "insert" | "into" | "update" | "delete" | "values"
    )
}

pub(crate) fn literal_value_present(buffer: &str, token_start: usize) -> bool {
    let before = buffer.get(..token_start).unwrap_or("").trim_end();
    let last_segment = before
        .rsplit_once(char::is_whitespace)
        .map_or(before, |(_, tail)| tail);
    !last_segment.is_empty()
        && last_segment
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric() || ch == '\'' || ch == '"')
}

fn token_priority(detail: &str, token: &str) -> u32 {
    if detail == "clause" {
        return clause_priority(token);
    }
    if detail == "logical" {
        return logical_priority(token);
    }
    if detail == "statement" {
        return statement_priority(token);
    }
    if detail == "ddl" {
        return ddl_priority(token);
    }
    if token.eq_ignore_ascii_case("JOIN") {
        return 2;
    }
    0
}

fn clause_priority(token: &str) -> u32 {
    if token.eq_ignore_ascii_case("WHERE") {
        return 8;
    }
    if token.eq_ignore_ascii_case("LIMIT") {
        return 7;
    }
    if token.eq_ignore_ascii_case("ORDER BY") {
        return 6;
    }
    if token.eq_ignore_ascii_case("GROUP BY") {
        return 5;
    }
    if token.eq_ignore_ascii_case("JOIN") {
        return 4;
    }
    if token.eq_ignore_ascii_case("OFFSET") {
        return 3;
    }
    if token.eq_ignore_ascii_case("RETURNING") {
        return 2;
    }
    1
}

fn logical_priority(token: &str) -> u32 {
    if token.eq_ignore_ascii_case("AND") {
        return 4;
    }
    if token.eq_ignore_ascii_case("OR") {
        return 3;
    }
    if token.eq_ignore_ascii_case("NOT") {
        return 2;
    }
    1
}

fn statement_priority(token: &str) -> u32 {
    match token.to_ascii_uppercase().as_str() {
        "SELECT" => 8,
        "UPDATE" => 7,
        "WITH" => 6,
        "INSERT INTO" => 5,
        "DELETE FROM" => 4,
        _ => 0,
    }
}

fn ddl_priority(token: &str) -> u32 {
    match token.to_ascii_uppercase().as_str() {
        "CREATE TABLE" => 3,
        "ALTER TABLE" => 2,
        "DROP TABLE" | "TRUNCATE TABLE" => 1,
        _ => 0,
    }
}
