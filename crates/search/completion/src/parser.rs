use pg_query::parse;

#[derive(Debug, Clone)]
pub(crate) struct ParsedQuery {
    select_tables: Vec<String>,
}

/// Parses the buffer with `pg_query` so completion can reuse discovered tables.
pub(crate) fn analyze_query(buffer: &str) -> Option<ParsedQuery> {
    if buffer.trim().is_empty() {
        return None;
    }
    let result = parse(buffer).ok()?;
    Some(ParsedQuery::from_parse_result(&result))
}

impl ParsedQuery {
    fn from_parse_result(result: &pg_query::ParseResult) -> Self {
        let mut select_tables = result.select_tables();
        if select_tables.is_empty() {
            // Fallback keeps UPDATE/DELETE tables visible when SELECT-specific list is empty.
            select_tables = result.tables();
        }
        Self { select_tables }
    }

    pub(crate) fn primary_table(&self) -> Option<&str> {
        self.select_tables.first().map(String::as_str)
    }
}
