use std::fmt::Write as FmtWrite;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use tokio_postgres::{
    types::{FromSql, Type},
    Row,
};
use uuid::Uuid;

use poqi_catalog::quote_identifier;

use crate::models::{
    PrimaryKeyValue, QueryResult, ResultMetadata, ResultOrigin, RowIdentity, RowIdentityPlan,
};

pub(crate) fn format_type_name(type_: &Type) -> String {
    let schema = type_.schema();
    if schema.is_empty() {
        quote_identifier(type_.name())
    } else {
        format!(
            "{}.{}",
            quote_identifier(schema),
            quote_identifier(type_.name())
        )
    }
}

pub(crate) fn rows_to_result(
    columns: &[String],
    column_types: &[String],
    rows: &[Row],
    identity_plan: Option<&RowIdentityPlan>,
) -> QueryResult {
    let mut display_columns = columns.to_owned();
    let mut display_types = column_types.to_owned();
    let identity_start = identity_plan.and_then(|plan| {
        columns
            .len()
            .checked_sub(3 + plan.primary_key.len())
            .inspect(|start| {
                display_columns.truncate(*start);
                display_types.truncate(*start);
            })
    });

    let mut metadata = ResultMetadata {
        source_table: None,
        row_identities: Vec::with_capacity(rows.len()),
        source_columns: vec![None; display_columns.len()],
        column_types: display_types,
        null_cells: Vec::with_capacity(rows.len()),
        origin: ResultOrigin::Unknown,
    };

    let formatted = rows
        .iter()
        .map(|row| {
            metadata
                .row_identities
                .push(decode_row_identity(row, identity_plan, identity_start));
            metadata.null_cells.push(
                (0..display_columns.len())
                    .map(|idx| row.get::<usize, NullProbe>(idx).0)
                    .collect(),
            );
            (0..display_columns.len())
                .map(|idx| format_value(row, idx))
                .collect()
        })
        .collect();

    QueryResult {
        columns: display_columns,
        rows: formatted,
        metadata,
    }
}

struct NullProbe(bool);

impl<'a> FromSql<'a> for NullProbe {
    fn from_sql(
        _ty: &Type,
        _raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(false))
    }

    fn from_sql_null(_ty: &Type) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(true))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

pub(crate) fn decode_row_identity(
    row: &Row,
    plan: Option<&RowIdentityPlan>,
    identity_start: Option<usize>,
) -> Option<RowIdentity> {
    let plan = plan?;
    let table_oid_index = identity_start?;
    let ctid_index = table_oid_index + 1;
    let xmin_index = table_oid_index + 2;
    let table_oid = row
        .try_get::<usize, Option<u32>>(table_oid_index)
        .ok()
        .flatten()?;
    let ctid = row
        .try_get::<usize, Option<String>>(ctid_index)
        .ok()
        .flatten()?;
    let xmin = row
        .try_get::<usize, Option<String>>(xmin_index)
        .ok()
        .flatten()?;
    let primary_key = plan
        .primary_key
        .iter()
        .enumerate()
        .map(|(offset, column)| {
            let index = xmin_index + 1 + offset;
            let type_oid = row.columns().get(index)?.type_().oid();
            let value = row.try_get::<usize, RawSqlValue>(index).ok()?.0;
            Some(PrimaryKeyValue {
                column: column.name.clone(),
                attribute_number: column.attribute_number,
                type_oid,
                value,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(RowIdentity {
        relation_oid: plan.relation_oid,
        table_oid,
        ctid,
        xmin,
        primary_key,
    })
}

struct RawSqlValue(Vec<u8>);

impl<'a> FromSql<'a> for RawSqlValue {
    fn from_sql(
        _ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(raw.to_vec()))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

fn format_value(row: &Row, idx: usize) -> String {
    let column_type = row.columns()[idx].type_();
    let column_name = row.columns()[idx].name();

    match *column_type {
        Type::BOOL => format_optional(row.get::<usize, Option<bool>>(idx)),
        Type::INT2 => format_optional(row.get::<usize, Option<i16>>(idx)),
        Type::INT4 => format_optional(row.get::<usize, Option<i32>>(idx)),
        Type::OID => format_oid(row, idx, column_name),
        Type::INT8 => format_optional(row.get::<usize, Option<i64>>(idx)),
        Type::FLOAT4 => format_optional(row.get::<usize, Option<f32>>(idx)),
        Type::FLOAT8 => format_optional(row.get::<usize, Option<f64>>(idx)),
        Type::NUMERIC => format_numeric(row, idx, column_name),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            format_text(row, idx, column_name, column_type)
        }
        Type::TIMESTAMP => format_timestamp(row, idx, column_name),
        Type::TIMESTAMPTZ => format_timestamptz(row, idx, column_name),
        Type::DATE => format_date(row, idx, column_name),
        Type::TIME => format_time(row, idx, column_name),
        Type::JSON | Type::JSONB => format_json(row, idx, column_name, column_type),
        Type::BYTEA => format_bytea(row, idx),
        Type::UUID => format_uuid(row, idx, column_name),
        _ => format_unknown(row, idx, column_name, column_type),
    }
}

fn format_oid(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<u32>>(idx) {
        Ok(value) => format_optional(value),
        Err(error) => {
            tracing::debug!(
                "Failed to deserialize OID column '{}' at index {}: {}",
                column_name,
                idx,
                error
            );
            "<oid>".to_string()
        }
    }
}

fn format_timestamp(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<NaiveDateTime>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize TIMESTAMP column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<TIMESTAMP>".to_string()
        }
    }
}

fn format_numeric(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<Decimal>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize NUMERIC column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<NUMERIC>".to_string()
        }
    }
}

fn format_timestamptz(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<DateTime<Utc>>>(idx) {
        Ok(value) => format_optional(value.map(|dt| dt.to_rfc3339())),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize TIMESTAMPTZ column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<TIMESTAMPTZ>".to_string()
        }
    }
}

fn format_date(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<NaiveDate>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize DATE column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<DATE>".to_string()
        }
    }
}

fn format_time(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<NaiveTime>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize TIME column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<TIME>".to_string()
        }
    }
}

fn format_text(row: &Row, idx: usize, column_name: &str, column_type: &Type) -> String {
    match row.try_get::<usize, Option<String>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize text column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            format!("<{}>", column_type.name())
        }
    }
}

fn format_json(row: &Row, idx: usize, column_name: &str, column_type: &Type) -> String {
    match row.try_get::<usize, Option<serde_json::Value>>(idx) {
        Ok(Some(value)) => value.to_string(),
        Ok(None) => "NULL".to_string(),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize JSON column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            format!("<{}>", column_type.name())
        }
    }
}

fn format_bytea(row: &Row, idx: usize) -> String {
    match row.get::<usize, Option<Vec<u8>>>(idx) {
        Some(bytes) => {
            let mut hex = String::with_capacity(bytes.len() * 2 + 2);
            hex.push_str("\\x");
            for byte in bytes {
                let _ = write!(hex, "{byte:02x}");
            }
            hex
        }
        None => "NULL".to_string(),
    }
}

fn format_uuid(row: &Row, idx: usize, column_name: &str) -> String {
    match row.try_get::<usize, Option<Uuid>>(idx) {
        Ok(value) => format_optional(value),
        Err(e) => {
            tracing::debug!(
                "Failed to deserialize UUID column '{}' at index {}: {}",
                column_name,
                idx,
                e
            );
            "<UUID>".to_string()
        }
    }
}

fn format_unknown(row: &Row, idx: usize, column_name: &str, column_type: &Type) -> String {
    if column_type.name().ends_with("[]") {
        "<ARRAY>".to_string()
    } else {
        match row.try_get::<usize, Option<String>>(idx) {
            Ok(value) => format_optional(value),
            Err(e) => {
                tracing::debug!(
                    "Failed to deserialize column '{}' of type {} at index {}: {}",
                    column_name,
                    column_type.name(),
                    idx,
                    e
                );
                format!("<{}>", column_type.name())
            }
        }
    }
}

fn format_optional<T>(value: Option<T>) -> String
where
    T: ToString,
{
    value.map_or_else(|| "NULL".to_string(), |v| v.to_string())
}
