#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SqlIdentifier {
    value: String,
    quoted: bool,
}

impl SqlIdentifier {
    pub(crate) fn matches_catalog_name(&self, catalog_name: &str) -> bool {
        if self.quoted {
            self.value == catalog_name
        } else {
            self.value.to_ascii_lowercase() == catalog_name
        }
    }

    pub(crate) fn matches_sql_identifier(&self, other: &Self) -> bool {
        match (self.quoted, other.quoted) {
            (true, true) => self.value == other.value,
            (false, false) => self.value.eq_ignore_ascii_case(&other.value),
            (true, false) => self.value == other.value.to_ascii_lowercase(),
            (false, true) => self.value.to_ascii_lowercase() == other.value,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScopedRelation {
    pub(crate) schema: Option<SqlIdentifier>,
    pub(crate) table: SqlIdentifier,
    pub(crate) alias: Option<SqlIdentifier>,
}

impl ScopedRelation {
    pub(crate) fn qualifier_matches(&self, qualifier: &SqlIdentifier) -> bool {
        self.alias.as_ref().map_or_else(
            || self.table.matches_sql_identifier(qualifier),
            |alias| alias.matches_sql_identifier(qualifier),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct QueryScope {
    pub(crate) relations: Vec<ScopedRelation>,
    pub(crate) has_relation_source: bool,
    pub(crate) select_list_has_star: bool,
}

impl QueryScope {
    pub(crate) fn has_qualifier(&self, raw_qualifier: &str) -> bool {
        parse_identifier(raw_qualifier).is_some_and(|qualifier| {
            self.relations
                .iter()
                .any(|relation| relation.qualifier_matches(&qualifier))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Identifier(SqlIdentifier),
    Dot,
    Comma,
    Star,
    OpenParen,
    CloseParen,
    Semicolon,
    Other,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
    depth: usize,
}

impl Token {
    fn keyword(&self, expected: &str) -> bool {
        matches!(
            &self.kind,
            TokenKind::Identifier(SqlIdentifier { value, quoted: false })
                if value.eq_ignore_ascii_case(expected)
        )
    }
}

pub(crate) fn analyze_query_scope(buffer: &str, cursor: usize) -> QueryScope {
    let cursor = clamp_boundary(buffer, cursor);
    let tokens = lex(buffer);
    let (statement_start, statement_end) = statement_bounds(&tokens, cursor);
    let cursor_depth = query_depth(&tokens, cursor, statement_start);
    let query_tokens: Vec<&Token> = tokens
        .iter()
        .filter(|token| {
            token.start >= statement_start
                && token.end <= statement_end
                && token.depth == cursor_depth
        })
        .collect();

    QueryScope {
        relations: collect_relations(&query_tokens),
        has_relation_source: query_tokens
            .iter()
            .any(|token| token.keyword("from") || token.keyword("join")),
        select_list_has_star: select_list_has_star(&query_tokens, cursor),
    }
}

fn query_depth(tokens: &[Token], cursor: usize, statement_start: usize) -> usize {
    let cursor_depth = depth_at_cursor(tokens, cursor);
    tokens
        .iter()
        .filter(|token| {
            token.start >= statement_start
                && token.start < cursor
                && token.depth <= cursor_depth
                && token.keyword("select")
        })
        .map(|token| token.depth)
        .next_back()
        .unwrap_or(cursor_depth)
}

pub(crate) fn parse_identifier(raw: &str) -> Option<SqlIdentifier> {
    let mut tokens = lex(raw);
    if tokens.len() != 1 {
        return None;
    }
    match tokens.remove(0).kind {
        TokenKind::Identifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn collect_relations(tokens: &[&Token]) -> Vec<ScopedRelation> {
    let mut relations = Vec::new();
    let mut idx = 0;
    while idx < tokens.len() {
        if !(tokens[idx].keyword("from") || tokens[idx].keyword("join")) {
            idx += 1;
            continue;
        }
        idx += 1;
        if let Some((relation, next)) = parse_relation(tokens, idx) {
            relations.push(relation);
            idx = next;
        }
    }
    relations
}

fn parse_relation(tokens: &[&Token], start: usize) -> Option<(ScopedRelation, usize)> {
    let first = identifier_at(tokens, start)?.clone();
    let mut idx = start + 1;
    let (schema, table) = if is_kind(tokens, idx, &TokenKind::Dot) {
        let table = identifier_at(tokens, idx + 1)?.clone();
        idx += 2;
        (Some(first), table)
    } else {
        (None, first)
    };

    let alias = if tokens.get(idx).is_some_and(|token| token.keyword("as")) {
        idx += 1;
        let alias = identifier_at(tokens, idx).cloned();
        idx += usize::from(alias.is_some());
        alias
    } else {
        identifier_at(tokens, idx)
            .filter(|identifier| !is_reserved_relation_word(identifier))
            .cloned()
            .inspect(|_| idx += 1)
    };

    Some((
        ScopedRelation {
            schema,
            table,
            alias,
        },
        idx,
    ))
}

fn identifier_at<'a>(tokens: &'a [&Token], idx: usize) -> Option<&'a SqlIdentifier> {
    match &tokens.get(idx)?.kind {
        TokenKind::Identifier(identifier) => Some(identifier),
        _ => None,
    }
}

fn is_kind(tokens: &[&Token], idx: usize, expected: &TokenKind) -> bool {
    tokens.get(idx).is_some_and(|token| &token.kind == expected)
}

fn is_reserved_relation_word(identifier: &SqlIdentifier) -> bool {
    if identifier.quoted {
        return false;
    }
    matches!(
        identifier.value.to_ascii_lowercase().as_str(),
        "where"
            | "join"
            | "inner"
            | "left"
            | "right"
            | "full"
            | "cross"
            | "natural"
            | "on"
            | "using"
            | "group"
            | "having"
            | "window"
            | "order"
            | "limit"
            | "offset"
            | "fetch"
            | "returning"
            | "union"
            | "intersect"
            | "except"
    )
}

fn select_list_has_star(tokens: &[&Token], cursor: usize) -> bool {
    let visible_tokens = tokens
        .iter()
        .copied()
        .filter(|token| token.start < cursor)
        .collect::<Vec<_>>();
    let mut in_select_list = false;
    let mut has_star = false;
    let mut select_index = 0;
    for (index, token) in visible_tokens.iter().enumerate() {
        if token.keyword("select") {
            in_select_list = true;
            has_star = false;
            select_index = index;
            continue;
        }
        if in_select_list && is_select_list_end(token) {
            in_select_list = false;
            continue;
        }
        if in_select_list
            && matches!(token.kind, TokenKind::Star)
            && is_projection_star(&visible_tokens, select_index, index)
        {
            has_star = true;
        }
    }
    has_star
}

fn is_projection_star(tokens: &[&Token], select_index: usize, star_index: usize) -> bool {
    let mut expression_start = star_index;
    if star_index >= 2
        && matches!(tokens[star_index - 1].kind, TokenKind::Dot)
        && matches!(tokens[star_index - 2].kind, TokenKind::Identifier(_))
    {
        expression_start -= 2;
        while expression_start >= 2
            && matches!(tokens[expression_start - 1].kind, TokenKind::Dot)
            && matches!(tokens[expression_start - 2].kind, TokenKind::Identifier(_))
        {
            expression_start -= 2;
        }
    }

    let Some(before_expression) = expression_start.checked_sub(1) else {
        return false;
    };
    before_expression == select_index
        || matches!(tokens[before_expression].kind, TokenKind::Comma)
        || (before_expression == select_index + 1
            && (tokens[before_expression].keyword("all")
                || tokens[before_expression].keyword("distinct")))
}

fn is_select_list_end(token: &Token) -> bool {
    [
        "from",
        "where",
        "group",
        "having",
        "window",
        "order",
        "limit",
        "offset",
        "fetch",
        "join",
        "union",
        "intersect",
        "except",
    ]
    .iter()
    .any(|keyword| token.keyword(keyword))
}

fn statement_bounds(tokens: &[Token], cursor: usize) -> (usize, usize) {
    let start = tokens
        .iter()
        .filter(|token| token.depth == 0 && token.end <= cursor)
        .filter(|token| matches!(token.kind, TokenKind::Semicolon))
        .map(|token| token.end)
        .next_back()
        .unwrap_or(0);
    let end = tokens
        .iter()
        .find(|token| {
            token.depth == 0 && token.start >= cursor && matches!(token.kind, TokenKind::Semicolon)
        })
        .map_or(usize::MAX, |token| token.start);
    (start, end)
}

fn depth_at_cursor(tokens: &[Token], cursor: usize) -> usize {
    let mut depth = 0_usize;
    for token in tokens.iter().filter(|token| token.start < cursor) {
        match token.kind {
            TokenKind::OpenParen => depth += 1,
            TokenKind::CloseParen => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn lex(input: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_token() {
        tokens.push(token);
    }
    tokens
}

struct Lexer<'a> {
    input: &'a str,
    idx: usize,
    depth: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            idx: 0,
            depth: 0,
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        while self.idx < self.input.len() {
            let start = self.idx;
            let ch = self.current_char()?;
            if ch.is_whitespace() {
                self.advance(ch);
                continue;
            }
            if self.starts_with("--") {
                self.skip_line_comment();
                continue;
            }
            if self.starts_with("/*") {
                self.skip_block_comment();
                continue;
            }
            if ch == '\'' {
                self.skip_single_quoted();
                continue;
            }
            if ch == '$' && self.skip_dollar_quoted() {
                continue;
            }
            if ch == '"' {
                return Some(self.quoted_identifier(start));
            }
            if identifier_start(ch) {
                return Some(self.unquoted_identifier(start));
            }
            self.advance(ch);
            let (kind, depth) = match ch {
                '(' => {
                    let depth = self.depth;
                    self.depth += 1;
                    (TokenKind::OpenParen, depth)
                }
                ')' => {
                    self.depth = self.depth.saturating_sub(1);
                    (TokenKind::CloseParen, self.depth)
                }
                '.' => (TokenKind::Dot, self.depth),
                ',' => (TokenKind::Comma, self.depth),
                '*' => (TokenKind::Star, self.depth),
                ';' => (TokenKind::Semicolon, self.depth),
                _ => (TokenKind::Other, self.depth),
            };
            return Some(Token {
                kind,
                start,
                end: self.idx,
                depth,
            });
        }
        None
    }

    fn quoted_identifier(&mut self, start: usize) -> Token {
        self.idx += 1;
        let mut value = String::new();
        while self.idx < self.input.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            if ch == '"' {
                self.idx += 1;
                if self.current_char() == Some('"') {
                    value.push('"');
                    self.idx += 1;
                    continue;
                }
                break;
            }
            value.push(ch);
            self.advance(ch);
        }
        Token {
            kind: TokenKind::Identifier(SqlIdentifier {
                value,
                quoted: true,
            }),
            start,
            end: self.idx,
            depth: self.depth,
        }
    }

    fn unquoted_identifier(&mut self, start: usize) -> Token {
        while let Some(ch) = self.current_char() {
            if !identifier_continue(ch) {
                break;
            }
            self.advance(ch);
        }
        Token {
            kind: TokenKind::Identifier(SqlIdentifier {
                value: self.input[start..self.idx].to_string(),
                quoted: false,
            }),
            start,
            end: self.idx,
            depth: self.depth,
        }
    }

    fn skip_line_comment(&mut self) {
        self.idx += 2;
        while let Some(ch) = self.current_char() {
            self.advance(ch);
            if ch == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        self.idx += 2;
        let mut nesting = 1_usize;
        while self.idx < self.input.len() && nesting > 0 {
            if self.starts_with("/*") {
                nesting += 1;
                self.idx += 2;
            } else if self.starts_with("*/") {
                nesting -= 1;
                self.idx += 2;
            } else if let Some(ch) = self.current_char() {
                self.advance(ch);
            }
        }
    }

    fn skip_single_quoted(&mut self) {
        self.idx += 1;
        while self.idx < self.input.len() {
            let Some(ch) = self.current_char() else {
                break;
            };
            self.advance(ch);
            if ch == '\'' {
                if self.current_char() == Some('\'') {
                    self.idx += 1;
                } else {
                    break;
                }
            }
        }
    }

    fn skip_dollar_quoted(&mut self) -> bool {
        let rest = &self.input[self.idx..];
        let Some(tail) = rest.get(1..).and_then(|value| value.find('$')) else {
            return false;
        };
        let delimiter_len = tail + 2;
        let delimiter = &rest[..delimiter_len];
        if !delimiter[1..delimiter.len() - 1]
            .chars()
            .all(identifier_continue)
        {
            return false;
        }
        self.idx += delimiter_len;
        if let Some(end) = self.input[self.idx..].find(delimiter) {
            self.idx += end + delimiter_len;
        } else {
            self.idx = self.input.len();
        }
        true
    }

    fn starts_with(&self, value: &str) -> bool {
        self.input[self.idx..].starts_with(value)
    }

    fn current_char(&self) -> Option<char> {
        self.input[self.idx..].chars().next()
    }

    fn advance(&mut self, ch: char) {
        self.idx += ch.len_utf8();
    }
}

fn identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn identifier_continue(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_alphanumeric()
}

fn clamp_boundary(input: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(input.len());
    while cursor > 0 && !input.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::{analyze_query_scope, parse_identifier};

    #[test]
    fn finds_ordinary_and_quoted_aliases_at_cursor_query_depth() {
        let sql = "SELECT w. FROM public.widgets AS w JOIN \"Odd.Schema\".\"Order Items\" \"o.i\" ON true";
        let cursor = sql.find("w.").expect("qualified select") + 2;
        let scope = analyze_query_scope(sql, cursor);

        assert_eq!(scope.relations.len(), 2);
        assert!(scope.relations[0].qualifier_matches(&parse_identifier("w").expect("alias")));
        assert!(scope.relations[1]
            .qualifier_matches(&parse_identifier("\"o.i\"").expect("quoted alias")));
    }

    #[test]
    fn ignores_sources_from_other_statements_and_nesting_levels() {
        let sql = "SELECT * FROM ignored; SELECT (SELECT * FROM nested) FROM current WHERE ";
        let scope = analyze_query_scope(sql, sql.len());
        assert_eq!(scope.relations.len(), 1);
        assert!(scope.relations[0].table.matches_catalog_name("current"));
    }

    #[test]
    fn star_detection_uses_tokens_instead_of_identifier_substrings() {
        let sql = "SELECT * AS selected ";
        let scope = analyze_query_scope(sql, sql.len());
        assert!(scope.select_list_has_star);
    }
}
