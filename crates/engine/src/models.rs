use serde::{Deserialize, Serialize};

use poqi_catalog::{QualifiedRelation, RelationKind};

/// CRUD-oriented commands supported by the engine worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrudAction {
    SelectTop {
        table: QualifiedRelation,
        limit: u32,
        relation_kind: RelationKind,
    },
    Insert {
        table: QualifiedRelation,
    },
    Update {
        table: QualifiedRelation,
    },
    Delete {
        table: QualifiedRelation,
    },
    UpdateCell {
        table: QualifiedRelation,
        column: String,
        new_value: Option<String>,
        row_identity: RowIdentity,
        column_type: Option<String>,
    },
    DeleteRow {
        table: QualifiedRelation,
        row_identity: RowIdentity,
    },
    RefreshRow {
        table: QualifiedRelation,
        row_identity: RowIdentity,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RowIdentity {
    /// OID of the relation named by the query, which can differ from `table_oid`
    /// for a row returned through a partitioned table.
    pub relation_oid: u32,
    pub table_oid: u32,
    pub ctid: String,
    pub xmin: String,
    pub primary_key: Vec<PrimaryKeyValue>,
}

/// An exact primary-key value retained in `PostgreSQL`'s binary wire format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimaryKeyValue {
    pub column: String,
    pub attribute_number: i16,
    pub type_oid: u32,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowIdentityPlan {
    pub relation_oid: u32,
    pub primary_key: Vec<PrimaryKeyColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrimaryKeyColumn {
    pub name: String,
    pub attribute_number: i16,
    pub type_oid: u32,
}

/// Metadata describing a single result set pushed back to the UI layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultMetadata {
    pub source_table: Option<QualifiedRelation>,
    pub row_identities: Vec<Option<RowIdentity>>,
    pub source_columns: Vec<Option<String>>,
    pub column_types: Vec<String>,
    #[serde(default)]
    pub null_cells: Vec<Vec<bool>>,
    pub origin: ResultOrigin,
}

impl Default for ResultMetadata {
    fn default() -> Self {
        Self {
            source_table: None,
            row_identities: Vec::new(),
            source_columns: Vec::new(),
            column_types: Vec::new(),
            null_cells: Vec::new(),
            origin: ResultOrigin::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ResultOrigin {
    #[default]
    Unknown,
    SelectTop {
        table: QualifiedRelation,
        limit: u32,
    },
    RunSql {
        sql: String,
        refresh: RunSqlRefresh,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunSqlRefresh {
    SimpleSingleTable { table: QualifiedRelation },
    ComplexSingleTable { table: QualifiedRelation },
}

impl RunSqlRefresh {
    #[must_use]
    pub fn table(&self) -> &QualifiedRelation {
        match self {
            Self::SimpleSingleTable { table } | Self::ComplexSingleTable { table } => table,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub metadata: ResultMetadata,
}

impl QueryResult {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            metadata: ResultMetadata::default(),
        }
    }
}
