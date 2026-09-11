use crate::{CompletionContext, CompletionItem, CompletionService, ALTER_TABLE_ACTIONS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlterTablePhase {
    AwaitingTable,
    AwaitingAction,
    AddColumn {
        has_column_kw: bool,
    },
    DropColumn {
        has_column_kw: bool,
    },
    RenameColumn {
        has_column_kw: bool,
        column_named: bool,
        saw_to: bool,
        has_target: bool,
    },
    RenameTable {
        saw_to: bool,
        has_target: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct AlterTableState<'a> {
    table_hint: Option<&'a str>,
    phase: AlterTablePhase,
}

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    let state = detect_state(ctx);
    match ctx.completion_context {
        CompletionContext::Table { ref schema_hint } => {
            service.collect_table_context(ctx.prefix, schema_hint.as_deref(), candidates);
            true
        }
        CompletionContext::Start
        | CompletionContext::Keyword
        | CompletionContext::General
        | CompletionContext::Literal => {
            collect_alter_table(service, ctx, state, candidates);
            true
        }
        _ => false,
    }
}

fn collect_alter_table(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    state: AlterTableState<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    match state.phase {
        AlterTablePhase::AwaitingTable => {
            service.collect_table_context(ctx.prefix, None, candidates);
        }
        AlterTablePhase::AwaitingAction => {
            CompletionService::collect_token_set(
                candidates,
                ctx.prefix,
                ALTER_TABLE_ACTIONS,
                "ddl",
            );
        }
        AlterTablePhase::AddColumn { has_column_kw } => {
            if !has_column_kw {
                CompletionService::collect_token_set(candidates, ctx.prefix, &["COLUMN"], "ddl");
            }
        }
        AlterTablePhase::DropColumn { has_column_kw } => {
            if has_column_kw {
                collect_table_columns(service, ctx.prefix, state.table_hint, candidates);
            } else {
                CompletionService::collect_token_set(candidates, ctx.prefix, &["COLUMN"], "ddl");
            }
        }
        AlterTablePhase::RenameColumn {
            has_column_kw,
            column_named,
            saw_to,
            has_target: _has_target,
        } => {
            if !has_column_kw {
                CompletionService::collect_token_set(candidates, ctx.prefix, &["COLUMN"], "ddl");
            }
            if saw_to {
                return;
            }
            if has_column_kw && !column_named {
                collect_table_columns(service, ctx.prefix, state.table_hint, candidates);
            }
            if column_named {
                CompletionService::collect_token_set(candidates, ctx.prefix, &["TO"], "ddl");
            }
        }
        AlterTablePhase::RenameTable {
            saw_to,
            has_target: _has_target,
        } => {
            if saw_to {
                return;
            }
            CompletionService::collect_token_set(candidates, ctx.prefix, &["TO"], "ddl");
        }
    }
}

fn detect_state(ctx: super::StatementContext<'_>) -> AlterTableState<'_> {
    let table_hint = CompletionService::merge_table_hint(ctx.hints, None);
    if table_hint.is_none() {
        return AlterTableState {
            table_hint,
            phase: AlterTablePhase::AwaitingTable,
        };
    }
    let action = ctx
        .scan
        .tokens
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, token)| matches!(token.as_str(), "add" | "drop" | "rename"));
    let Some((idx, action)) = action else {
        return AlterTableState {
            table_hint,
            phase: AlterTablePhase::AwaitingAction,
        };
    };
    match action.as_str() {
        "add" => AlterTableState {
            table_hint,
            phase: AlterTablePhase::AddColumn {
                has_column_kw: ctx.scan.tokens.get(idx + 1).map(String::as_str) == Some("column"),
            },
        },
        "drop" => AlterTableState {
            table_hint,
            phase: AlterTablePhase::DropColumn {
                has_column_kw: ctx.scan.tokens.get(idx + 1).map(String::as_str) == Some("column"),
            },
        },
        "rename" => detect_rename_state(ctx, table_hint, idx),
        _ => AlterTableState {
            table_hint,
            phase: AlterTablePhase::AwaitingAction,
        },
    }
}

fn detect_rename_state<'a>(
    ctx: super::StatementContext<'a>,
    table_hint: Option<&'a str>,
    rename_idx: usize,
) -> AlterTableState<'a> {
    let next = ctx.scan.tokens.get(rename_idx + 1).map(String::as_str);
    if next == Some("to") {
        return AlterTableState {
            table_hint,
            phase: AlterTablePhase::RenameTable {
                saw_to: true,
                has_target: ctx.scan.tokens.get(rename_idx + 2).is_some(),
            },
        };
    }
    let has_column_kw = next == Some("column");
    let subject_idx = rename_idx + 1 + usize::from(has_column_kw);
    let column_named = ctx
        .scan
        .tokens
        .get(subject_idx)
        .is_some_and(|token| token != "to");
    let saw_to = ctx
        .scan
        .tokens
        .get(subject_idx + 1)
        .is_some_and(|token| token == "to");
    let has_target = ctx.scan.tokens.get(subject_idx + 2).is_some();
    AlterTableState {
        table_hint,
        phase: AlterTablePhase::RenameColumn {
            has_column_kw,
            column_named,
            saw_to,
            has_target,
        },
    }
}

fn collect_table_columns(
    service: &CompletionService,
    prefix: &str,
    table_hint: Option<&str>,
    candidates: &mut Vec<CompletionItem>,
) {
    service.collect_columns(candidates, prefix, table_hint);
}
