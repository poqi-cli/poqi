use std::collections::{HashMap, HashSet};

use poqi_catalog::{display_identifier, CatalogSnapshot};

use crate::analyzer::{parse_identifier, QueryScope, ScopedRelation};

/// Read-only snapshot of catalog entities used during completion.
#[derive(Debug, Clone)]
pub(crate) struct CompletionMetadata {
    schemas: Vec<SchemaEntry>,
    tables: Vec<TableEntry>,
    columns: Vec<ColumnEntry>,
    columns_by_table: HashMap<String, Vec<ColumnEntry>>,
    ambiguous_tables: HashSet<String>,
}

impl CompletionMetadata {
    pub(crate) fn new(snapshot: CatalogSnapshot) -> Self {
        let mut schema_map: HashMap<String, SchemaEntry> = HashMap::new();
        let tables: Vec<TableEntry> = snapshot
            .tables
            .into_iter()
            .map(|table| {
                let schema = table.schema.clone();
                let schema_lower = schema.to_ascii_lowercase();
                schema_map
                    .entry(schema_lower.clone())
                    .or_insert_with(|| SchemaEntry {
                        name: schema.clone(),
                        lower: schema_lower.clone(),
                    });
                let name = table.name.clone();
                let qualified_name = format!("{schema}.{name}");
                TableEntry {
                    schema,
                    schema_lower,
                    name: name.clone(),
                    lower: qualified_name.to_ascii_lowercase(),
                    name_lower: name.to_ascii_lowercase(),
                }
            })
            .collect();
        let mut schemas: Vec<SchemaEntry> = schema_map.into_values().collect();
        schemas.sort_by(|a, b| a.name.cmp(&b.name));

        let mut table_name_counts: HashMap<String, usize> = HashMap::new();
        for table in &tables {
            *table_name_counts
                .entry(table.name_lower.clone())
                .or_default() += 1;
        }
        let ambiguous_tables: HashSet<String> = table_name_counts
            .into_iter()
            .filter_map(|(name, count)| (count > 1).then_some(name))
            .collect();

        let mut columns: Vec<ColumnEntry> = Vec::with_capacity(snapshot.columns.len());
        let mut columns_by_table: HashMap<String, Vec<ColumnEntry>> = HashMap::new();
        for column in snapshot.columns {
            let schema = column.schema.clone();
            let table = column.table.clone();
            let name = column.name.clone();
            let entry = ColumnEntry {
                schema,
                table,
                name: name.clone(),
                name_lower: name.to_ascii_lowercase(),
            };
            columns.push(entry.clone());

            // Store both schema-qualified and bare table keys so aliases still resolve to column lists.
            let qualified_key = format!("{}.{}", column.schema, column.table).to_ascii_lowercase();
            columns_by_table
                .entry(qualified_key)
                .or_default()
                .push(entry.clone());
            columns_by_table
                .entry(column.table.to_ascii_lowercase())
                .or_default()
                .push(entry);
        }

        Self {
            schemas,
            tables,
            columns,
            columns_by_table,
            ambiguous_tables,
        }
    }

    pub(crate) fn schemas(&self) -> &[SchemaEntry] {
        &self.schemas
    }

    pub(crate) fn tables(&self) -> &[TableEntry] {
        &self.tables
    }

    pub(crate) fn columns(&self) -> &[ColumnEntry] {
        &self.columns
    }

    pub(crate) fn schema_exists(&self, name: &str) -> bool {
        parse_identifier(name).is_some_and(|identifier| {
            self.schemas
                .iter()
                .any(|schema| identifier.matches_catalog_name(&schema.name))
        })
    }

    pub(crate) fn find_table(&self, schema: Option<&str>, name: &str) -> Option<&TableEntry> {
        let table_lower = name.to_ascii_lowercase();
        let schema_lower = schema.map(str::to_ascii_lowercase);
        self.tables.iter().find(|table| {
            table.name_lower == table_lower
                && schema_lower
                    .as_ref()
                    .is_none_or(|schema| table.schema_lower == *schema)
        })
    }

    /// Returns every column matching a schema-qualified or bare table/alias.
    pub(crate) fn columns_for_table(&self, name: &str) -> Option<&[ColumnEntry]> {
        let key = name.to_ascii_lowercase();
        self.columns_by_table.get(&key).map(Vec::as_slice)
    }

    pub(crate) fn columns_for_scope(&self, scope: &QueryScope) -> Vec<&ColumnEntry> {
        let mut seen = HashSet::new();
        scope
            .relations
            .iter()
            .filter_map(|relation| self.find_scoped_table(relation))
            .flat_map(|table| {
                self.columns.iter().filter(move |column| {
                    column.schema == table.schema && column.table == table.name
                })
            })
            .filter(|column| {
                seen.insert((
                    column.schema.as_str(),
                    column.table.as_str(),
                    column.name.as_str(),
                ))
            })
            .collect()
    }

    pub(crate) fn columns_for_qualifier(
        &self,
        scope: &QueryScope,
        raw_qualifier: &str,
    ) -> Vec<&ColumnEntry> {
        let Some(qualifier) = parse_identifier(raw_qualifier) else {
            return Vec::new();
        };
        let matching_relations: Vec<&ScopedRelation> = scope
            .relations
            .iter()
            .filter(|relation| relation.qualifier_matches(&qualifier))
            .collect();
        let table = if matching_relations.len() == 1 {
            self.find_scoped_table(matching_relations[0])
        } else if matching_relations.is_empty() && !scope.has_relation_source {
            self.find_scoped_table(&ScopedRelation {
                schema: None,
                table: qualifier,
                alias: None,
            })
        } else {
            None
        };
        table.map_or_else(Vec::new, |table| {
            self.columns
                .iter()
                .filter(|column| column.schema == table.schema && column.table == table.name)
                .collect()
        })
    }

    fn find_scoped_table(&self, relation: &ScopedRelation) -> Option<&TableEntry> {
        let matches: Vec<&TableEntry> = self
            .tables
            .iter()
            .filter(|table| relation.table.matches_catalog_name(&table.name))
            .filter(|table| {
                relation
                    .schema
                    .as_ref()
                    .is_none_or(|schema| schema.matches_catalog_name(&table.schema))
            })
            .collect();
        (matches.len() == 1).then(|| matches[0])
    }

    pub(crate) fn table_display_for_column(&self, column: &ColumnEntry) -> String {
        let table_lower = column.table.to_ascii_lowercase();
        if self.ambiguous_tables.contains(&table_lower) {
            format!(
                "{}.{}",
                display_identifier(&column.schema),
                display_identifier(&column.table)
            )
        } else {
            display_identifier(&column.table)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SchemaEntry {
    /// Display name used in completions.
    pub(crate) name: String,
    /// Cached lowercase form for cheap prefix filtering.
    pub(crate) lower: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TableEntry {
    pub(crate) schema: String,
    pub(crate) schema_lower: String,
    pub(crate) name: String,
    /// Lowercase fully-qualified name for comparisons.
    pub(crate) lower: String,
    /// Lowercase table-only name for schema-specific scans.
    pub(crate) name_lower: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ColumnEntry {
    pub(crate) schema: String,
    pub(crate) table: String,
    pub(crate) name: String,
    /// Lowercase column name used for scoring.
    pub(crate) name_lower: String,
}
