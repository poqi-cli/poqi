use crate::CompletionService;
use poqi_catalog::{CatalogSnapshot, ColumnDefault, ColumnMeta, Nullability, TableId, TableMeta};

pub(crate) fn demo_snapshot() -> CatalogSnapshot {
    CatalogSnapshot {
        tables: vec![TableMeta {
            relation_kind: poqi_catalog::RelationKind::Table,
            id: TableId("public.widgets".to_string()),
            name: "widgets".to_string(),
            schema: "public".to_string(),
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
    }
}

pub(crate) fn completion_service() -> CompletionService {
    CompletionService::new(demo_snapshot())
}

pub(crate) fn multi_schema_snapshot() -> CatalogSnapshot {
    let mut snapshot = demo_snapshot();
    snapshot.tables.push(TableMeta {
        relation_kind: poqi_catalog::RelationKind::Table,
        id: TableId("analytics.gadgets".to_string()),
        name: "gadgets".to_string(),
        schema: "analytics".to_string(),
    });
    snapshot.columns.push(ColumnMeta {
        schema: "analytics".to_string(),
        table: "gadgets".to_string(),
        name: "code".to_string(),
        data_type: "text".to_string(),
        ordinal_position: 1,
        nullability: Nullability::NotNull,
        character_maximum_length: None,
        numeric_precision: None,
        numeric_scale: None,
        is_primary_key: true,
        default_kind: ColumnDefault::None,
        is_foreign_key: false,
    });
    snapshot.tables.push(TableMeta {
        relation_kind: poqi_catalog::RelationKind::Table,
        id: TableId("analytics.widgets".to_string()),
        name: "widgets".to_string(),
        schema: "analytics".to_string(),
    });
    snapshot.columns.push(ColumnMeta {
        schema: "analytics".to_string(),
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
    });
    snapshot
}

pub(crate) fn multi_schema_service() -> CompletionService {
    CompletionService::new(multi_schema_snapshot())
}

pub(crate) fn deceptive_metadata_service() -> CompletionService {
    let bidi_table = "audit\u{202E}cod";
    let literal_escape_table = r"audit\u{202E}cod";
    CompletionService::new(CatalogSnapshot {
        tables: vec![
            TableMeta {
                relation_kind: poqi_catalog::RelationKind::Table,
                id: TableId("bidi".to_string()),
                name: bidi_table.to_string(),
                schema: "public".to_string(),
            },
            TableMeta {
                relation_kind: poqi_catalog::RelationKind::Table,
                id: TableId("literal".to_string()),
                name: literal_escape_table.to_string(),
                schema: "public".to_string(),
            },
        ],
        columns: vec![ColumnMeta {
            schema: "public".to_string(),
            table: bidi_table.to_string(),
            name: "line\nitem".to_string(),
            data_type: "text".to_string(),
            ordinal_position: 1,
            nullability: Nullability::Nullable,
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            is_primary_key: false,
            default_kind: ColumnDefault::None,
            is_foreign_key: false,
        }],
        foreign_keys: Vec::new(),
    })
}
