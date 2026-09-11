use super::lexer::{WithLexer, WithToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WithState {
    /// `WITH` or `WITH RECURSIVE` just typed, waiting for first CTE name.
    ExpectName,
    /// Inside `WITH cte (...)` column list before `AS`.
    ColumnList,
    /// CTE name typed, waiting for `AS`.
    ExpectAs,
    /// Waiting for a materialization modifier after `AS`.
    ExpectMaterialized { after_not: bool },
    /// `AS` typed, waiting for `(`.
    ExpectOpenParen,
    /// Inside `(...)`.
    InsideQuery { body_kind: Option<CteBodyKind> },
    /// After `)`, waiting for `,` (next CTE) or statement keywords.
    AfterQuery { has_statement: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CteBodyKind {
    Select,
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializedParse {
    AwaitingParen(usize),
    ExpectMaterialized { after_not: bool },
    NeedParen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CteScanOutcome {
    /// Parsing can continue at the returned index.
    Advance(usize),
    /// A partial token stream points to a user-facing completion state.
    NeedState(WithState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyScanResult {
    /// Body closed cleanly; `next_idx` points to the token after `)`.
    Complete {
        body_kind: Option<CteBodyKind>,
        next_idx: usize,
    },
    /// Body is still open, so completions should target the inner query.
    Incomplete { body_kind: Option<CteBodyKind> },
}

/// One-pass parser that tracks a single WITH clause and returns the next completion state.
struct WithClauseAnalyzer<'a> {
    tokens: &'a [WithToken<'a>],
    idx: usize,
}

impl<'a> WithClauseAnalyzer<'a> {
    fn new(tokens: &'a [WithToken<'a>]) -> Self {
        Self { tokens, idx: 0 }
    }

    fn analyze(mut self) -> WithState {
        let Some(start_idx) = self.locate_with_clause() else {
            return WithState::AfterQuery {
                has_statement: true,
            };
        };
        self.idx = start_idx;
        self.walk_ctes()
    }

    fn walk_ctes(&mut self) -> WithState {
        loop {
            match self.parse_cte() {
                CteScanOutcome::Advance(next_idx) => {
                    self.idx = next_idx;
                    if self.idx >= self.tokens.len() {
                        return WithState::AfterQuery {
                            has_statement: false,
                        };
                    }
                    if matches!(self.tokens[self.idx], WithToken::Comma) {
                        self.idx += 1;
                        continue;
                    }
                    return WithState::AfterQuery {
                        has_statement: true,
                    };
                }
                CteScanOutcome::NeedState(state) => return state,
            }
        }
    }

    fn locate_with_clause(&self) -> Option<usize> {
        let mut idx = self.tokens.iter().position(|token| token.eq_word("with"))? + 1;
        if idx < self.tokens.len() && self.tokens[idx].eq_word("recursive") {
            idx += 1;
        }
        Some(idx)
    }

    /// Walks a single CTE definition and stops as soon as we can surface a completion state.
    fn parse_cte(&self) -> CteScanOutcome {
        let idx = match self.parse_cte_name_and_columns(self.idx) {
            Ok(next_idx) => next_idx,
            Err(state) => return CteScanOutcome::NeedState(state),
        };

        let idx = match self.parse_as_keyword(idx) {
            Ok(next_idx) => next_idx,
            Err(state) => return CteScanOutcome::NeedState(state),
        };

        let idx = match self.materialized_state(idx) {
            MaterializedParse::AwaitingParen(next_idx) => next_idx,
            MaterializedParse::ExpectMaterialized { after_not } => {
                return CteScanOutcome::NeedState(WithState::ExpectMaterialized { after_not });
            }
            MaterializedParse::NeedParen => {
                return CteScanOutcome::NeedState(WithState::ExpectOpenParen);
            }
        };

        let idx = match self.parse_open_paren(idx) {
            Ok(next_idx) => next_idx,
            Err(state) => return CteScanOutcome::NeedState(state),
        };

        match self.scan_cte_body(idx) {
            BodyScanResult::Complete { next_idx, .. } => CteScanOutcome::Advance(next_idx),
            BodyScanResult::Incomplete { body_kind } => {
                CteScanOutcome::NeedState(WithState::InsideQuery { body_kind })
            }
        }
    }

    /// Validates the CTE name and, when present, its column list balance.
    fn parse_cte_name_and_columns(&self, mut idx: usize) -> Result<usize, WithState> {
        if idx >= self.tokens.len() || !self.tokens[idx].is_word() {
            return Err(WithState::ExpectName);
        }
        idx += 1;
        if idx < self.tokens.len() && matches!(self.tokens[idx], WithToken::OpenParen) {
            idx += 1;
            idx = self.skip_column_list(idx)?;
        }
        Ok(idx)
    }

    fn skip_column_list(&self, mut idx: usize) -> Result<usize, WithState> {
        let mut depth = 1;
        while idx < self.tokens.len() && depth > 0 {
            match self.tokens[idx] {
                WithToken::OpenParen => depth += 1,
                WithToken::CloseParen => depth -= 1,
                _ => {}
            }
            idx += 1;
        }
        if depth > 0 {
            Err(WithState::ColumnList)
        } else {
            Ok(idx)
        }
    }

    fn parse_as_keyword(&self, idx: usize) -> Result<usize, WithState> {
        if idx >= self.tokens.len() {
            return Err(WithState::ExpectAs);
        }
        if !self.tokens[idx].eq_word("as") {
            if self.tokens[idx].starts_with("as") {
                return Err(WithState::ExpectAs);
            }
            return Err(WithState::ExpectAs);
        }
        Ok(idx + 1)
    }

    fn materialized_state(&self, mut idx: usize) -> MaterializedParse {
        let mut awaiting_materialized = false;
        loop {
            if idx >= self.tokens.len() {
                return if awaiting_materialized {
                    MaterializedParse::ExpectMaterialized { after_not: true }
                } else {
                    MaterializedParse::NeedParen
                };
            }
            match self.tokens[idx] {
                WithToken::Word(word) => {
                    if awaiting_materialized {
                        if word.eq_ignore_ascii_case("materialized") {
                            return MaterializedParse::AwaitingParen(idx + 1);
                        }
                        if word.to_ascii_lowercase().starts_with("materialized") {
                            return MaterializedParse::ExpectMaterialized { after_not: true };
                        }
                        return MaterializedParse::ExpectMaterialized { after_not: true };
                    }
                    if word.eq_ignore_ascii_case("not") {
                        awaiting_materialized = true;
                        idx += 1;
                        continue;
                    }
                    let lower = word.to_ascii_lowercase();
                    if lower.starts_with("not") {
                        return MaterializedParse::ExpectMaterialized { after_not: false };
                    }
                    if lower.starts_with("materialized") && lower.len() != "materialized".len() {
                        return MaterializedParse::ExpectMaterialized { after_not: false };
                    }
                    if word.eq_ignore_ascii_case("materialized") {
                        return MaterializedParse::AwaitingParen(idx + 1);
                    }
                }
                _ => {
                    return if awaiting_materialized {
                        MaterializedParse::ExpectMaterialized { after_not: true }
                    } else {
                        MaterializedParse::AwaitingParen(idx)
                    };
                }
            }
            break;
        }
        MaterializedParse::AwaitingParen(idx)
    }

    fn parse_open_paren(&self, idx: usize) -> Result<usize, WithState> {
        if idx >= self.tokens.len() {
            return Err(WithState::ExpectOpenParen);
        }
        if !matches!(self.tokens[idx], WithToken::OpenParen) {
            return Err(WithState::ExpectOpenParen);
        }
        Ok(idx + 1)
    }

    /// Tracks the CTE body to either the closing paren or the incomplete depth.
    fn scan_cte_body(&self, mut idx: usize) -> BodyScanResult {
        let mut depth = 1;
        let mut body_kind = None;
        while idx < self.tokens.len() {
            match self.tokens[idx] {
                WithToken::OpenParen => depth += 1,
                WithToken::CloseParen => {
                    depth -= 1;
                    idx += 1;
                    if depth == 0 {
                        return BodyScanResult::Complete {
                            body_kind,
                            next_idx: idx,
                        };
                    }
                    continue;
                }
                WithToken::Word(word) => {
                    if depth == 1 && body_kind.is_none() {
                        body_kind = CteBodyKind::from_word(word);
                    }
                }
                WithToken::Comma => {}
            }
            idx += 1;
        }
        BodyScanResult::Incomplete { body_kind }
    }
}

/// Scans the current buffer slice to understand which portion of the WITH clause is being typed.
pub(crate) fn analyze_with_state(ctx: crate::start_keywords::StatementContext<'_>) -> WithState {
    let slice = ctx
        .buffer
        .get(..ctx.cursor.min(ctx.buffer.len()))
        .unwrap_or("");
    let statement = slice
        .rsplit_once(';')
        .map_or(slice, |(_, segment)| segment)
        .trim_start();
    let tokens: Vec<_> = WithLexer::new(statement).collect();
    if tokens.is_empty() {
        return WithState::ExpectName;
    }
    WithClauseAnalyzer::new(&tokens).analyze()
}

impl CteBodyKind {
    fn from_word(word: &str) -> Option<Self> {
        if word.eq_ignore_ascii_case("select") {
            return Some(Self::Select);
        }
        if word.eq_ignore_ascii_case("insert") {
            return Some(Self::Insert);
        }
        if word.eq_ignore_ascii_case("update") {
            return Some(Self::Update);
        }
        if word.eq_ignore_ascii_case("delete") {
            return Some(Self::Delete);
        }
        None
    }
}
