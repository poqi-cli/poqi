use smallvec::SmallVec;

use crate::analyzer::QueryScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Table,
    Column,
    Snippet,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub insert_text: String,
    pub detail: String,
    pub kind: CompletionKind,
    pub score: u32,
}

#[derive(Debug, Default, Clone)]
pub struct CompletionBatch {
    pub items: SmallVec<[CompletionItem; 16]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    Start,
    Table { schema_hint: Option<String> },
    Column { table_hint: Option<String> },
    SelectList,
    FromSource,
    JoinType,
    JoinCondition,
    Where,
    GroupBy,
    Having,
    Window,
    OrderBy,
    Limit,
    Offset,
    Returning,
    InsertValues,
    UpdateSet,
    Keyword,
    General,
    Literal,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ContextHints<'a> {
    pub(crate) parser_table_hint: Option<&'a str>,
    pub(crate) scan_table_hint: Option<&'a str>,
    pub(crate) token_is_empty: bool,
    pub(crate) last_token: Option<&'a str>,
    pub(crate) scope: &'a QueryScope,
}
