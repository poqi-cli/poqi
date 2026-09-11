use crate::{
    analyzer::{QueryScope, SqlScan},
    CompletionContext, CompletionItem, CompletionService, ContextHints,
};

pub(crate) mod alter_table;
pub(crate) mod create_table;
pub(crate) mod delete_from;
pub(crate) mod drop_table;
pub(crate) mod insert_into;
pub(crate) mod select;
pub(crate) mod shared;
pub(crate) mod truncate_table;
pub(crate) mod update;
pub(crate) mod with;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartKeyword {
    Select,
    Update,
    With,
    InsertInto,
    DeleteFrom,
    CreateTable,
    AlterTable,
    DropTable,
    TruncateTable,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StatementContext<'a> {
    pub(crate) buffer: &'a str,
    pub(crate) cursor: usize,
    pub(crate) token_start: usize,
    pub(crate) completion_context: &'a CompletionContext,
    pub(crate) prefix: &'a str,
    pub(crate) hints: ContextHints<'a>,
    pub(crate) scan: &'a SqlScan,
    pub(crate) scope: &'a QueryScope,
}

pub(crate) fn detect_start_keyword(tokens: &[String]) -> Option<StartKeyword> {
    let first = tokens.first()?.as_str();
    match first {
        "select" => Some(StartKeyword::Select),
        "update" => Some(StartKeyword::Update),
        "with" => Some(StartKeyword::With),
        "insert" => Some(StartKeyword::InsertInto),
        "delete" => Some(StartKeyword::DeleteFrom),
        "create" if tokens.get(1).map(String::as_str) == Some("table") => {
            Some(StartKeyword::CreateTable)
        }
        "alter" if tokens.get(1).map(String::as_str) == Some("table") => {
            Some(StartKeyword::AlterTable)
        }
        "drop" if tokens.get(1).map(String::as_str) == Some("table") => {
            Some(StartKeyword::DropTable)
        }
        "truncate" if tokens.get(1).map(String::as_str) == Some("table") => {
            Some(StartKeyword::TruncateTable)
        }
        _ => None,
    }
}

pub(crate) fn dispatch_start_keyword(
    service: &CompletionService,
    keyword: StartKeyword,
    context: StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    match keyword {
        StartKeyword::Select => select::suggest(service, context, candidates),
        StartKeyword::Update => update::suggest(service, context, candidates),
        StartKeyword::With => with::suggest(service, context, candidates),
        StartKeyword::InsertInto => insert_into::suggest(service, context, candidates),
        StartKeyword::DeleteFrom => delete_from::suggest(service, context, candidates),
        StartKeyword::CreateTable => create_table::suggest(service, context, candidates),
        StartKeyword::AlterTable => alter_table::suggest(service, context, candidates),
        StartKeyword::DropTable => drop_table::suggest(service, context, candidates),
        StartKeyword::TruncateTable => truncate_table::suggest(service, context, candidates),
    }
}

pub(crate) fn tokens_since_keyword<'a>(tokens: &'a [String], keyword: &str) -> &'a [String] {
    tokens
        .iter()
        .rposition(|token| token == keyword)
        .map_or(&[], |idx| &tokens[idx..])
}
