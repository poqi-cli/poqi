#![warn(clippy::all, clippy::pedantic)]

mod display;

pub use display::{display_identifier, identifier_requires_explicit_preview};

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use poqi_db::{Database, DatabaseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const TABLES_QUERY: &str = "
    SELECT namespace.nspname,
           relation.relname,
           relation.relkind::text
    FROM pg_catalog.pg_class relation
    JOIN pg_catalog.pg_namespace namespace
      ON namespace.oid = relation.relnamespace
    WHERE relation.relkind IN ('r', 'p', 'v', 'm', 'f')
      AND namespace.nspname NOT IN ('pg_catalog', 'information_schema')
      AND NOT pg_catalog.pg_is_other_temp_schema(namespace.oid)
      AND (
          pg_catalog.pg_has_role(relation.relowner, 'USAGE')
          OR pg_catalog.has_table_privilege(
              relation.oid,
              'SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER'
          )
          OR pg_catalog.has_any_column_privilege(
              relation.oid,
              'SELECT, INSERT, UPDATE, REFERENCES'
          )
      )
    ORDER BY namespace.nspname, relation.relname
";

const COLUMNS_QUERY: &str = "
    SELECT namespace.nspname,
           relation.relname,
           attribute.attname,
           COALESCE(columns.data_type, pg_catalog.format_type(attribute.atttypid, attribute.atttypmod)),
           attribute.attnum::integer,
           CASE
               WHEN columns.column_name IS NOT NULL THEN columns.is_nullable = 'YES'
               ELSE NOT (attribute.attnotnull OR attribute_type.typnotnull)
           END,
           columns.character_maximum_length::integer,
           columns.numeric_precision::integer,
           columns.numeric_scale::integer,
           CASE
               WHEN columns.column_name IS NOT NULL THEN columns.column_default
               ELSE pg_catalog.pg_get_expr(defaults.adbin, defaults.adrelid)
           END,
           COALESCE(
               columns.is_generated,
               CASE WHEN attribute.attgenerated <> '' THEN 'ALWAYS' ELSE 'NEVER' END
           ),
           COALESCE(
               columns.is_identity,
               CASE WHEN attribute.attidentity <> '' THEN 'YES' ELSE 'NO' END
           ),
           COALESCE(
               columns.identity_generation,
               CASE attribute.attidentity
                   WHEN 'a' THEN 'ALWAYS'
                   WHEN 'd' THEN 'BY DEFAULT'
                   ELSE NULL
               END
           )
    FROM pg_catalog.pg_class relation
    JOIN pg_catalog.pg_namespace namespace
      ON namespace.oid = relation.relnamespace
    JOIN pg_catalog.pg_attribute attribute
      ON attribute.attrelid = relation.oid
     AND attribute.attnum > 0
     AND NOT attribute.attisdropped
    JOIN pg_catalog.pg_type attribute_type
      ON attribute_type.oid = attribute.atttypid
    LEFT JOIN pg_catalog.pg_attrdef defaults
      ON defaults.adrelid = relation.oid
     AND defaults.adnum = attribute.attnum
    LEFT JOIN information_schema.columns columns
      ON columns.table_schema = namespace.nspname
     AND columns.table_name = relation.relname
     AND columns.column_name = attribute.attname
    WHERE relation.relkind IN ('r', 'p', 'v', 'm', 'f')
      AND namespace.nspname NOT IN ('pg_catalog', 'information_schema')
      AND NOT pg_catalog.pg_is_other_temp_schema(namespace.oid)
      AND (
          pg_catalog.pg_has_role(relation.relowner, 'USAGE')
          OR pg_catalog.has_column_privilege(
              relation.oid,
              attribute.attnum,
              'SELECT, INSERT, UPDATE, REFERENCES'
          )
      )
    ORDER BY namespace.nspname, relation.relname, attribute.attnum
";

const PRIMARY_KEYS_QUERY: &str = "
    SELECT kc.table_schema,
           kc.table_name,
           kc.column_name
    FROM information_schema.table_constraints tc
    JOIN information_schema.key_column_usage kc
      ON kc.constraint_name = tc.constraint_name
     AND kc.table_catalog = tc.table_catalog
     AND kc.table_schema = tc.table_schema
     AND kc.table_name = tc.table_name
     AND kc.constraint_schema = tc.constraint_schema
     AND kc.constraint_catalog = tc.constraint_catalog
    WHERE tc.constraint_type = 'PRIMARY KEY'
      AND kc.table_schema NOT IN ('pg_catalog', 'information_schema')
    ORDER BY kc.table_schema, kc.table_name, kc.ordinal_position
";

const FOREIGN_KEYS_QUERY: &str = "
    SELECT rc.constraint_name,
           kcu.table_schema,
           kcu.table_name,
           kcu.column_name,
           ukcu.table_schema AS referenced_schema,
           ukcu.table_name AS referenced_table,
           ukcu.column_name AS referenced_column
    FROM information_schema.referential_constraints rc
    JOIN information_schema.key_column_usage kcu
      ON kcu.constraint_name = rc.constraint_name
     AND kcu.constraint_schema = rc.constraint_schema
     AND kcu.constraint_catalog = rc.constraint_catalog
    JOIN information_schema.key_column_usage ukcu
      ON ukcu.constraint_name = rc.unique_constraint_name
     AND ukcu.constraint_schema = rc.unique_constraint_schema
     AND ukcu.constraint_catalog = rc.unique_constraint_catalog
     AND ukcu.ordinal_position = kcu.position_in_unique_constraint
    WHERE kcu.table_schema NOT IN ('pg_catalog', 'information_schema')
    ORDER BY kcu.table_schema, kcu.table_name, kcu.ordinal_position
";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TableId(pub String);

/// A `PostgreSQL` relation name whose components remain lossless until SQL rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct QualifiedRelation {
    pub schema: Option<String>,
    pub name: String,
}

impl QualifiedRelation {
    #[must_use]
    pub fn new(schema: Option<String>, name: String) -> Self {
        Self { schema, name }
    }

    #[must_use]
    pub fn in_schema(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(Some(schema.into()), name.into())
    }

    #[must_use]
    pub fn unqualified(name: impl Into<String>) -> Self {
        Self::new(None, name.into())
    }

    /// Render a SQL-safe relation name, preserving unqualified `search_path` lookup.
    #[must_use]
    pub fn quoted(&self) -> String {
        self.schema.as_ref().map_or_else(
            || quote_identifier(&self.name),
            |schema| {
                format!(
                    "{}.{}",
                    quote_identifier(schema),
                    quote_identifier(&self.name)
                )
            },
        )
    }

    /// Render an unambiguous readable label for UI and diagnostics.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.schema.as_ref().map_or_else(
            || quote_identifier(&display_identifier(&self.name)),
            |schema| {
                format!(
                    "{}.{}",
                    quote_identifier(&display_identifier(schema)),
                    quote_identifier(&display_identifier(&self.name))
                )
            },
        )
    }

    /// Whether automatic row preview should be withheld until the user
    /// explicitly selects this relation.
    #[must_use]
    pub fn requires_explicit_preview(&self) -> bool {
        self.schema
            .as_deref()
            .is_some_and(identifier_requires_explicit_preview)
            || identifier_requires_explicit_preview(&self.name)
    }
}

/// Quote one `PostgreSQL` identifier component and escape embedded quotes.
#[must_use]
pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub id: TableId,
    pub name: String,
    pub schema: String,
    pub relation_kind: RelationKind,
}

impl TableMeta {
    #[must_use]
    pub fn relation(&self) -> QualifiedRelation {
        QualifiedRelation::in_schema(self.schema.clone(), self.name.clone())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationKind {
    Table,
    PartitionedTable,
    View,
    MaterializedView,
    ForeignTable,
}

impl RelationKind {
    #[must_use]
    pub fn supports_row_identity(self) -> bool {
        matches!(self, Self::Table | Self::PartitionedTable)
    }

    #[must_use]
    pub fn from_pg_relkind(value: &str) -> Option<Self> {
        match value {
            "r" => Some(Self::Table),
            "p" => Some(Self::PartitionedTable),
            "v" => Some(Self::View),
            "m" => Some(Self::MaterializedView),
            "f" => Some(Self::ForeignTable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub data_type: String,
    pub ordinal_position: i32,
    pub nullability: Nullability,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub is_primary_key: bool,
    pub default_kind: ColumnDefault,
    pub is_foreign_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Nullability {
    Nullable,
    NotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ColumnDefault {
    None,
    Default,
    Identity,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyMeta {
    pub constraint_name: String,
    pub schema: String,
    pub table: String,
    pub column: String,
    pub referenced_schema: String,
    pub referenced_table: String,
    pub referenced_column: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogSnapshot {
    pub tables: Vec<TableMeta>,
    pub columns: Vec<ColumnMeta>,
    pub foreign_keys: Vec<ForeignKeyMeta>,
}

/// Groups tables by schema name with table lists sorted alphabetically.
#[must_use]
pub fn schema_table_map(snapshot: &CatalogSnapshot) -> HashMap<String, Vec<String>> {
    let mut schemas: HashMap<String, Vec<String>> = HashMap::new();
    for table in &snapshot.tables {
        schemas
            .entry(table.schema.clone())
            .or_default()
            .push(table.name.clone());
    }
    for tables in schemas.values_mut() {
        tables.sort();
    }
    schemas
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("database error: {0}")]
    Database(#[from] DatabaseError),
}

#[derive(Debug, Clone)]
pub struct Catalog {
    database: Database,
    tables: Vec<TableMeta>,
    columns: Vec<ColumnMeta>,
    foreign_keys: Vec<ForeignKeyMeta>,
}

impl Catalog {
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self {
            database,
            tables: Vec::new(),
            columns: Vec::new(),
            foreign_keys: Vec::new(),
        }
    }

    /// Refresh the cached table metadata from the connected database.
    ///
    /// # Errors
    /// Returns an error if the underlying database query fails.
    pub async fn refresh(&mut self) -> Result<(), CatalogError> {
        let rows = self.database.query(TABLES_QUERY, &[]).await?;
        self.tables = rows
            .into_iter()
            .filter_map(|row| {
                let schema: String = row.get(0);
                let name: String = row.get(1);
                let relkind: String = row.get(2);
                let relation_kind = RelationKind::from_pg_relkind(&relkind)?;
                Some(TableMeta {
                    id: TableId(QualifiedRelation::in_schema(&schema, &name).quoted()),
                    name,
                    schema,
                    relation_kind,
                })
            })
            .collect();

        let primary_key_rows = self.database.query(PRIMARY_KEYS_QUERY, &[]).await?;
        let primary_keys: HashSet<(String, String, String)> = primary_key_rows
            .into_iter()
            .map(|row| (row.get(0), row.get(1), row.get(2)))
            .collect();

        let foreign_key_rows = self.database.query(FOREIGN_KEYS_QUERY, &[]).await?;
        self.foreign_keys = foreign_key_rows
            .iter()
            .map(|row| ForeignKeyMeta {
                constraint_name: row.get(0),
                schema: row.get(1),
                table: row.get(2),
                column: row.get(3),
                referenced_schema: row.get(4),
                referenced_table: row.get(5),
                referenced_column: row.get(6),
            })
            .collect();
        let foreign_keys: HashSet<(String, String, String)> = self
            .foreign_keys
            .iter()
            .map(|fk| (fk.schema.clone(), fk.table.clone(), fk.column.clone()))
            .collect();

        let column_rows = self.database.query(COLUMNS_QUERY, &[]).await?;
        self.columns = column_rows
            .into_iter()
            .map(|row| {
                let schema: String = row.get(0);
                let table: String = row.get(1);
                let name: String = row.get(2);
                let key = (schema.clone(), table.clone(), name.clone());
                let is_nullable: bool = row.get(5);
                let column_default: Option<String> = row.get(9);
                let is_generated = row.get::<_, String>(10) == "ALWAYS";
                let is_identity = matches!(
                    row.get::<_, Option<String>>(12).as_deref(),
                    Some("ALWAYS" | "BY DEFAULT")
                ) || row.get::<_, String>(11) == "YES";
                let default_kind = if is_generated {
                    ColumnDefault::Generated
                } else if is_identity {
                    ColumnDefault::Identity
                } else if column_default.is_some() {
                    ColumnDefault::Default
                } else {
                    ColumnDefault::None
                };
                let nullability = if is_nullable {
                    Nullability::Nullable
                } else {
                    Nullability::NotNull
                };
                ColumnMeta {
                    schema,
                    table,
                    name,
                    data_type: row.get(3),
                    ordinal_position: row.get::<_, i32>(4),
                    nullability,
                    character_maximum_length: row.get(6),
                    numeric_precision: row.get(7),
                    numeric_scale: row.get(8),
                    is_primary_key: primary_keys.contains(&key),
                    default_kind,
                    is_foreign_key: foreign_keys.contains(&key),
                }
            })
            .collect();

        Ok(())
    }

    pub fn tables(&self) -> impl Iterator<Item = &TableMeta> {
        self.tables.iter()
    }

    pub fn columns(&self) -> impl Iterator<Item = &ColumnMeta> {
        self.columns.iter()
    }

    #[must_use]
    pub fn snapshot(&self) -> CatalogSnapshot {
        CatalogSnapshot {
            tables: self.tables.clone(),
            columns: self.columns.clone(),
            foreign_keys: self.foreign_keys.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
