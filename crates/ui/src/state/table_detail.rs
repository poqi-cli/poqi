use std::collections::{HashMap, HashSet};

use poqi_catalog::{CatalogSnapshot, ColumnDefault, Nullability, QualifiedRelation};

#[derive(Debug, Clone)]
pub(crate) struct TableDetailState {
    cache: HashMap<QualifiedRelation, TableDetail>,
    open_table: Option<QualifiedRelation>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableDetail {
    pub(crate) table: String,
    pub(crate) columns: Vec<TableColumnDetail>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableColumnDetail {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) ordinal_position: i32,
    pub(crate) max_length: Option<i32>,
    pub(crate) numeric_precision: Option<i32>,
    pub(crate) numeric_scale: Option<i32>,
    pub(crate) nullability: Nullability,
    pub(crate) default_kind: ColumnDefault,
    pub(crate) is_primary_key: bool,
    pub(crate) is_foreign_key: bool,
}

impl TableDetailState {
    #[must_use]
    pub(crate) fn new(snapshot: CatalogSnapshot) -> Self {
        let cache = build_cache(snapshot);
        Self {
            cache,
            open_table: None,
        }
    }

    #[must_use]
    pub(crate) fn open_for(&mut self, table: &QualifiedRelation) -> Option<&TableDetail> {
        if self.cache.contains_key(table) {
            self.open_table = Some(table.clone());
        } else {
            self.open_table = None;
        }
        self.current()
    }

    pub(crate) fn close(&mut self) {
        self.open_table = None;
    }

    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.open_table.is_some()
    }

    #[must_use]
    pub(crate) fn current(&self) -> Option<&TableDetail> {
        self.open_table
            .as_ref()
            .and_then(|table| self.cache.get(table))
    }

    pub(crate) fn follow_selection(&mut self, table: Option<&QualifiedRelation>) {
        if self.open_table.is_none() {
            return;
        }
        let Some(table) = table else {
            self.close();
            return;
        };
        if self.cache.contains_key(table) {
            self.open_table = Some(table.clone());
        } else {
            self.close();
        }
    }

    pub(crate) fn replace(&mut self, snapshot: CatalogSnapshot) {
        let open_table = self.open_table.clone();
        self.cache = build_cache(snapshot);
        if let Some(table) = open_table {
            if self.cache.contains_key(&table) {
                self.open_table = Some(table);
            } else {
                self.close();
            }
        } else {
            self.close();
        }
    }
}

fn build_cache(snapshot: CatalogSnapshot) -> HashMap<QualifiedRelation, TableDetail> {
    let fk_columns: HashSet<(String, String, String)> = snapshot
        .foreign_keys
        .iter()
        .map(|fk| (fk.schema.clone(), fk.table.clone(), fk.column.clone()))
        .collect();

    let mut cache: HashMap<QualifiedRelation, TableDetail> = snapshot
        .tables
        .into_iter()
        .map(|table| {
            let schema = table.schema;
            let name = table.name;
            let key = QualifiedRelation::in_schema(schema, name);
            let detail = TableDetail {
                table: key.display_name(),
                columns: Vec::new(),
            };
            (key, detail)
        })
        .collect();

    for column in snapshot.columns {
        let key = QualifiedRelation::in_schema(&column.schema, &column.table);
        let is_fk = column.is_foreign_key
            || fk_columns.contains(&(
                column.schema.clone(),
                column.table.clone(),
                column.name.clone(),
            ));
        let detail = cache.entry(key.clone()).or_insert(TableDetail {
            table: key.display_name(),
            columns: Vec::new(),
        });
        detail.columns.push(TableColumnDetail {
            name: column.name,
            data_type: column.data_type,
            ordinal_position: column.ordinal_position,
            max_length: column.character_maximum_length,
            numeric_precision: column.numeric_precision,
            numeric_scale: column.numeric_scale,
            nullability: column.nullability,
            default_kind: column.default_kind,
            is_primary_key: column.is_primary_key,
            is_foreign_key: is_fk,
        });
    }

    for detail in cache.values_mut() {
        detail.columns.sort_by_key(|column| column.ordinal_position);
    }

    cache
}

#[cfg(test)]
mod tests {
    use super::*;
    use poqi_catalog::{ColumnDefault, ColumnMeta, Nullability, RelationKind, TableId, TableMeta};

    #[test]
    fn opens_and_tracks_selection() {
        let snapshot = CatalogSnapshot {
            tables: vec![TableMeta {
                id: TableId("public.widgets".to_string()),
                name: "widgets".to_string(),
                schema: "public".to_string(),
                relation_kind: RelationKind::Table,
            }],
            columns: vec![ColumnMeta {
                schema: "public".to_string(),
                table: "widgets".to_string(),
                name: "id".to_string(),
                data_type: "integer".to_string(),
                ordinal_position: 1,
                nullability: Nullability::NotNull,
                character_maximum_length: None,
                numeric_precision: None,
                numeric_scale: None,
                is_primary_key: true,
                default_kind: ColumnDefault::None,
                is_foreign_key: false,
            }],
            foreign_keys: Vec::new(),
        };
        let mut state = TableDetailState::new(snapshot);
        assert!(!state.is_open());

        let widgets = QualifiedRelation::in_schema("public", "widgets");
        assert!(state.open_for(&widgets).is_some());
        assert!(state.is_open());
        let column = state.current().unwrap().columns.first().unwrap();
        assert_eq!(column.name, "id");
        assert!(column.is_primary_key);
        assert!(!column.is_foreign_key);
        assert_eq!(column.default_kind, ColumnDefault::None);
        assert_eq!(column.nullability, Nullability::NotNull);

        state.follow_selection(Some(&widgets));
        assert!(state.is_open());

        let missing = QualifiedRelation::in_schema("public", "missing");
        state.follow_selection(Some(&missing));
        assert!(!state.is_open());
    }
}
