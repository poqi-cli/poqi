use poqi_engine::QueryResult;
use poqi_search_semantic::format_row_prompt;

/// Flatten rows into prompts the embedder expects.
pub(super) fn build_documents(result: &QueryResult, title_column: Option<&str>) -> Vec<String> {
    let title_index = title_column.and_then(|candidate| {
        result
            .columns
            .iter()
            .position(|col| col.eq_ignore_ascii_case(candidate))
    });
    let mut docs = Vec::with_capacity(result.rows.len());
    for (row_index, row) in result.rows.iter().enumerate() {
        let nulls = result.metadata.null_cells.get(row_index);
        let mut cols = Vec::with_capacity(result.columns.len());
        for (column_index, (name, value)) in result.columns.iter().zip(row.iter()).enumerate() {
            let is_null = nulls
                .and_then(|mask| mask.get(column_index))
                .copied()
                .unwrap_or(false);
            cols.push((name.as_str(), value.as_str(), is_null));
        }
        let title = title_index
            .filter(|idx| {
                !nulls
                    .and_then(|mask| mask.get(*idx))
                    .copied()
                    .unwrap_or(false)
            })
            .and_then(|idx| row.get(idx))
            .map(String::as_str);
        docs.push(format_row_prompt(title, &cols));
    }
    docs
}

#[cfg(test)]
mod tests {
    use super::build_documents;
    use poqi_engine::{QueryResult, ResultMetadata};

    #[test]
    fn documents_distinguish_sql_null_from_literal_null_text() {
        let result = QueryResult {
            columns: vec!["actual".into(), "literal".into()],
            rows: vec![vec!["NULL".into(), "NULL".into()]],
            metadata: ResultMetadata {
                null_cells: vec![vec![true, false]],
                ..ResultMetadata::default()
            },
        };

        let documents = build_documents(&result, Some("actual"));
        assert_eq!(documents, vec!["actual: [NULL] | literal: NULL"]);
    }
}
