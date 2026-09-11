use poqi_catalog::{display_identifier, quote_identifier};

use crate::analyzer::QueryScope;
use crate::metadata::{ColumnEntry, CompletionMetadata};
use crate::scoring::match_score;
use crate::types::{CompletionItem, CompletionKind};

/// Caps predicate suggestions so WHERE lists stay focused.
pub(crate) const MAX_PREDICATE_SNIPPETS: usize = 8;

pub(crate) fn collect_columns(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    table_hint: Option<&str>,
) {
    let columns = columns_for_hint(metadata, table_hint);
    collect_column_items(metadata, columns.iter(), items, prefix);
}

pub(crate) fn collect_columns_in_scope(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    scope: &QueryScope,
) {
    let columns = metadata.columns_for_scope(scope);
    collect_column_items(metadata, columns, items, prefix);
}

pub(crate) fn collect_qualified_columns(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    scope: &QueryScope,
    qualifier: &str,
) {
    let columns = metadata.columns_for_qualifier(scope, qualifier);
    collect_column_items(metadata, columns, items, prefix);
}

fn collect_column_items<'a>(
    metadata: &CompletionMetadata,
    columns: impl IntoIterator<Item = &'a ColumnEntry>,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
) {
    items.extend(
        columns
            .into_iter()
            .filter_map(|column| {
                match_score(prefix, &column.name_lower).map(|score| {
                    let table_detail = table_display_for_column(metadata, column);
                    CompletionItem {
                        label: display_identifier(&column.name),
                        insert_text: quote_identifier(&column.name),
                        detail: table_detail,
                        kind: CompletionKind::Column,
                        score: score.saturating_sub(1),
                    }
                })
            })
            .collect::<Vec<_>>(),
    );
}

pub(crate) fn columns_for_hint<'a>(
    metadata: &'a CompletionMetadata,
    table_hint: Option<&str>,
) -> &'a [ColumnEntry] {
    table_hint
        .filter(|name| !name.is_empty())
        .and_then(|table| metadata.columns_for_table(table))
        .unwrap_or_else(|| metadata.columns())
}

pub(crate) fn table_display_for_column(
    metadata: &CompletionMetadata,
    column: &ColumnEntry,
) -> String {
    metadata.table_display_for_column(column)
}

pub(crate) fn collect_predicate_snippets(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    table_hint: Option<&str>,
) {
    let columns = columns_for_hint(metadata, table_hint);
    collect_predicate_items(metadata, items, prefix, columns.iter());
}

pub(crate) fn collect_predicate_snippets_in_scope(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    scope: &QueryScope,
) {
    let columns = metadata.columns_for_scope(scope);
    collect_predicate_items(metadata, items, prefix, columns);
}

fn collect_predicate_items<'a>(
    metadata: &CompletionMetadata,
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    columns: impl IntoIterator<Item = &'a ColumnEntry>,
) {
    let mut added = 0usize;
    for column in columns {
        if let Some(score) = match_score(prefix, &column.name_lower) {
            let snippet_score = score.saturating_sub(1);
            let table_detail = metadata.table_display_for_column(column);
            items.push(predicate_snippet_item(column, &table_detail, snippet_score));
            added += 1;
        }
        if added >= MAX_PREDICATE_SNIPPETS {
            break;
        }
    }
}

fn predicate_snippet_item(column: &ColumnEntry, table_detail: &str, score: u32) -> CompletionItem {
    CompletionItem {
        label: format!("{} = ?", display_identifier(&column.name)),
        insert_text: format!("{} = ", quote_identifier(&column.name)),
        detail: format!("{table_detail}, predicate"),
        kind: CompletionKind::Snippet,
        score,
    }
}
