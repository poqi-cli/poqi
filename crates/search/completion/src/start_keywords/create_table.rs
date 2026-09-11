use crate::{CompletionContext, CompletionItem, CompletionKind, CompletionService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateTablePhase {
    AwaitingName,
    TableNamed {
        has_open_paren: bool,
        has_close_paren: bool,
        has_body: bool,
    },
}

pub(crate) fn suggest(
    _service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    let state = detect_state(ctx);
    match ctx.completion_context {
        CompletionContext::Start => {
            if matches!(state, CreateTablePhase::AwaitingName) {
                CompletionService::collect_statement_keywords(ctx.prefix, candidates);
                CompletionService::collect_start_ddl_keywords(ctx, candidates);
            } else {
                collect_create_table(ctx, state, candidates);
            }
            true
        }
        CompletionContext::Table { schema_hint: _ }
        | CompletionContext::Keyword
        | CompletionContext::General
        | CompletionContext::Literal => {
            collect_create_table(ctx, state, candidates);
            true
        }
        _ => false,
    }
}

fn collect_create_table(
    ctx: super::StatementContext<'_>,
    state: CreateTablePhase,
    candidates: &mut Vec<CompletionItem>,
) {
    match state {
        CreateTablePhase::AwaitingName => {}
        CreateTablePhase::TableNamed {
            has_open_paren,
            has_close_paren,
            has_body,
        } => {
            let at_table_start = ctx.scan.tokens.len() == 2 && ctx.hints.token_is_empty;
            let after_name = ctx.scan.tokens.len() == 3 && ctx.hints.token_is_empty;
            if !has_open_paren && !has_close_paren && (at_table_start || after_name) {
                candidates.push(CompletionItem {
                    label: "(".to_string(),
                    insert_text: "(".to_string(),
                    detail: "ddl".to_string(),
                    kind: CompletionKind::Keyword,
                    score: 0,
                });
            }
            if !has_close_paren && has_body && ctx.prefix.is_empty() {
                candidates.push(CompletionItem {
                    label: ");".to_string(),
                    insert_text: ");".to_string(),
                    detail: "ddl".to_string(),
                    kind: CompletionKind::Keyword,
                    score: 0,
                });
            }
        }
    }
}

fn detect_state(ctx: super::StatementContext<'_>) -> CreateTablePhase {
    let has_name = ctx.scan.tokens.len() >= 2;
    let slice = ctx
        .buffer
        .get(..ctx.cursor.min(ctx.buffer.len()))
        .unwrap_or("");
    let segment = {
        let lower = slice.to_ascii_lowercase();
        if let Some(idx) = lower.rfind("create table") {
            &slice[idx + "create table".len()..]
        } else {
            slice
        }
    };
    let has_open_paren = segment.contains('(');
    let has_close_paren = segment.contains(')');
    let has_body = segment
        .split('(')
        .nth(1)
        .is_some_and(|rest| rest.contains(|ch: char| ch.is_ascii_alphanumeric()));
    if has_name {
        CreateTablePhase::TableNamed {
            has_open_paren,
            has_close_paren,
            has_body,
        }
    } else {
        CreateTablePhase::AwaitingName
    }
}
