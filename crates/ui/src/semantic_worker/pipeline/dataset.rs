use std::path::{Path, PathBuf};

use poqi_engine::{QueryResult, ResultOrigin};
use poqi_search_semantic::NULL_SENTINEL;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RowKey([u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum OriginSignature {
    SelectTop { table: String, limit: u32 },
    RunSqlSingleTable { table: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModelFingerprint {
    model_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct DatasetId {
    origin: OriginSignature,
    columns_hash: [u8; 32],
    title_column: Option<String>,
    model: ModelFingerprint,
    sentinel: &'static str,
}

pub(super) fn build_dataset_id(
    result: &QueryResult,
    model_path: &Path,
    title_column: Option<&str>,
) -> DatasetId {
    DatasetId {
        origin: origin_signature(result),
        columns_hash: hash_columns(&result.columns),
        title_column: title_column.map(str::to_ascii_lowercase),
        model: ModelFingerprint {
            model_path: model_path.to_path_buf(),
        },
        sentinel: NULL_SENTINEL,
    }
}

pub(super) fn row_keys(result: &QueryResult) -> Vec<RowKey> {
    result
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let nulls = result.metadata.null_cells.get(index).map(Vec::as_slice);
            RowKey(hash_row(row, &result.columns, nulls))
        })
        .collect()
}

fn hash_columns(columns: &[String]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for column in columns {
        update_hash(&mut hasher, column);
    }
    hasher.finalize().into()
}

fn hash_row(row: &[String], columns: &[String], nulls: Option<&[bool]>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for (index, (col, value)) in columns.iter().zip(row.iter()).enumerate() {
        update_hash(&mut hasher, col);
        update_hash(&mut hasher, value);
        hasher.update([u8::from(
            nulls
                .and_then(|mask| mask.get(index))
                .copied()
                .unwrap_or(false),
        )]);
    }
    hasher.finalize().into()
}

fn update_hash(hasher: &mut Sha256, value: &str) {
    let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
    hasher.update(len.to_le_bytes());
    hasher.update(value.as_bytes());
}

fn origin_signature(result: &QueryResult) -> OriginSignature {
    match &result.metadata.origin {
        ResultOrigin::SelectTop { table, limit } => OriginSignature::SelectTop {
            table: table.display_name(),
            limit: *limit,
        },
        ResultOrigin::RunSql { refresh, .. } => OriginSignature::RunSqlSingleTable {
            table: refresh.table().display_name(),
        },
        ResultOrigin::Unknown => {
            result
                .metadata
                .source_table
                .as_ref()
                .map_or(OriginSignature::Unknown, |table| {
                    OriginSignature::RunSqlSingleTable {
                        table: table.display_name(),
                    }
                })
        }
    }
}

#[cfg(test)]
mod tests;
