use poqi_catalog::{display_identifier, quote_identifier};

use crate::analyzer::parse_identifier;
use crate::metadata::CompletionMetadata;
use crate::scoring::match_score;
use crate::types::{CompletionItem, CompletionKind};

pub(crate) fn collect_tables(
    items: &mut Vec<CompletionItem>,
    metadata: &CompletionMetadata,
    prefix: &str,
    schema_hint: Option<&str>,
    allow_schema_prefix: bool,
) {
    if let Some(schema) = schema_hint.filter(|name| !name.is_empty()) {
        let Some(schema_identifier) = parse_identifier(schema) else {
            return;
        };
        let matches = metadata
            .tables()
            .iter()
            .filter(|table| schema_identifier.matches_catalog_name(&table.schema))
            .filter_map(|table| {
                match_score(prefix, &table.name_lower).map(|score| CompletionItem {
                    label: display_identifier(&table.name),
                    insert_text: quote_identifier(&table.name),
                    detail: "table".to_string(),
                    kind: CompletionKind::Table,
                    score,
                })
            })
            .collect::<Vec<_>>();
        items.extend(matches);
        return;
    }

    if allow_schema_prefix && metadata.schemas().len() > 1 {
        let schema_matches = metadata
            .schemas()
            .iter()
            .filter_map(|schema| {
                match_score(prefix, &schema.lower).map(|score| CompletionItem {
                    label: display_identifier(&schema.name),
                    insert_text: format!("{}.", quote_identifier(&schema.name)),
                    detail: "schema".to_string(),
                    kind: CompletionKind::Table,
                    score,
                })
            })
            .collect::<Vec<_>>();
        if !schema_matches.is_empty() {
            items.extend(schema_matches);
            return;
        }
    }

    if prefix.is_empty() && !allow_schema_prefix {
        return;
    }

    let matches = metadata
        .tables()
        .iter()
        .filter_map(|table| {
            match_score(prefix, &table.lower).map(|score| CompletionItem {
                label: format!(
                    "{}.{}",
                    display_identifier(&table.schema),
                    display_identifier(&table.name)
                ),
                insert_text: format!(
                    "{}.{}",
                    quote_identifier(&table.schema),
                    quote_identifier(&table.name)
                ),
                detail: format!("schema: {}", display_identifier(&table.schema)),
                kind: CompletionKind::Table,
                score,
            })
        })
        .collect::<Vec<_>>();
    items.extend(matches);
}
