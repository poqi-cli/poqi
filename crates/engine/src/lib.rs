#![warn(clippy::all, clippy::pedantic)]

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use poqi_catalog::{quote_identifier, Catalog, CatalogSnapshot, QualifiedRelation, RelationKind};
use poqi_db::{Database, DatabaseError};
use thiserror::Error;
use tokio_postgres::types::{private::BytesMut, IsNull, ToSql, Type};

mod constants;
mod models;
mod query_analysis;
mod transactions;
mod value_format;

pub use constants::{CTID_ALIAS, TABLEOID_ALIAS, XMIN_ALIAS};
pub use models::{
    CrudAction, PrimaryKeyValue, QueryResult, ResultMetadata, ResultOrigin, RowIdentity,
    RunSqlRefresh,
};

use models::{PrimaryKeyColumn, RowIdentityPlan};

use value_format::rows_to_result;

// Re-exported types live in `models.rs`.

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("engine execution is not yet implemented")]
    NotImplemented,
    #[error("database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("crud action {0:?} is not supported")]
    UnsupportedCrud(Box<CrudAction>),
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
    #[error("no matching row found for edit/delete operation")]
    NoMatchingRow,
    #[error("the selected row changed or its primary-key identity is no longer valid")]
    StaleRow,
    #[error("SQL batches are not supported; submit one statement at a time (received {0})")]
    UnsupportedSqlBatch(usize),
}

/// Returns whether an anyhow error chain contains `PostgreSQL`'s structured query-canceled code.
///
/// SQLSTATE 57014 covers both user-requested cancellations and statement timeouts. Callers must
/// use their operation context to decide whether suppressing the error is appropriate.
#[must_use]
pub fn is_query_canceled(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        source.downcast_ref::<DatabaseError>().is_some_and(|error| {
            matches!(error, DatabaseError::OperationCanceled)
                || matches!(error, DatabaseError::Postgres(postgres) if postgres_is_query_canceled(postgres))
        }) || source
            .downcast_ref::<tokio_postgres::Error>()
            .is_some_and(postgres_is_query_canceled)
    })
}

fn postgres_is_query_canceled(error: &tokio_postgres::Error) -> bool {
    error.code() == Some(&tokio_postgres::error::SqlState::QUERY_CANCELED)
}

impl ToSql for PrimaryKeyValue {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        if ty.oid() != self.type_oid {
            return Err(format!(
                "primary-key type changed from OID {} to OID {}",
                self.type_oid,
                ty.oid()
            )
            .into());
        }
        out.extend_from_slice(&self.value);
        Ok(IsNull::No)
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }

    tokio_postgres::types::to_sql_checked!();
}

#[async_trait]
pub trait Engine: Send + Sync {
    async fn execute(&self, sql: &str) -> Result<QueryResult>;
    async fn crud(&self, action: CrudAction) -> Result<QueryResult>;
    async fn refresh_catalog(&self) -> Result<CatalogSnapshot>;
    async fn cancel_current(&self) -> Result<bool>;
}

#[derive(Debug, Default)]
pub struct StubEngine;

#[async_trait]
impl Engine for StubEngine {
    async fn execute(&self, _sql: &str) -> Result<QueryResult> {
        Err(EngineError::NotImplemented.into())
    }

    async fn crud(&self, _action: CrudAction) -> Result<QueryResult> {
        Err(EngineError::NotImplemented.into())
    }

    async fn refresh_catalog(&self) -> Result<CatalogSnapshot> {
        Err(EngineError::NotImplemented.into())
    }

    async fn cancel_current(&self) -> Result<bool> {
        Ok(false)
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseEngine {
    database: Database,
    statement_timeout: Duration,
    page_size: u32,
}

impl DatabaseEngine {
    #[must_use]
    pub fn new(database: Database, statement_timeout: Duration, page_size: u32) -> Self {
        Self {
            database,
            statement_timeout,
            page_size: page_size.max(1),
        }
    }

    async fn query(
        &self,
        sql: &str,
        identity_plan: Option<&RowIdentityPlan>,
        read_only: bool,
    ) -> Result<QueryResult, EngineError> {
        let (column_names, column_types, rows) =
            self.query_with_params_mode(sql, &[], read_only).await?;
        Ok(rows_to_result(
            &column_names,
            &column_types,
            &rows,
            identity_plan,
        ))
    }

    async fn row_identity_plan(
        &self,
        relation: &QualifiedRelation,
    ) -> Result<Option<RowIdentityPlan>, EngineError> {
        const QUALIFIED_LOOKUP: &str =
            "pg_catalog.to_regclass(pg_catalog.format('%I.%I', $1::text, $2::text))";
        const UNQUALIFIED_LOOKUP: &str =
            "pg_catalog.to_regclass(pg_catalog.format('%I', $1::text))";
        let query = |lookup: &str| {
            format!(
                "SELECT relation.oid::oid,
                        relation.relkind::text,
                        attribute.attname,
                        attribute.attnum::smallint,
                        attribute.atttypid::oid,
                        pg_catalog.has_column_privilege(
                            relation.oid, attribute.attnum, 'SELECT'
                        ),
                        relation.relkind = 'p'
                            OR NOT EXISTS (
                                SELECT 1
                                FROM pg_catalog.pg_inherits inheritance
                                WHERE inheritance.inhparent = relation.oid
                            )
                 FROM pg_catalog.pg_class relation
                 JOIN pg_catalog.pg_index identity_index
                   ON identity_index.indrelid = relation.oid
                  AND identity_index.indisprimary
                  AND identity_index.indisvalid
                  AND identity_index.indisready
                 CROSS JOIN LATERAL
                    pg_catalog.unnest(identity_index.indkey::smallint[])
                    WITH ORDINALITY AS key_column(attribute_number, key_ordinality)
                 JOIN pg_catalog.pg_attribute attribute
                   ON attribute.attrelid = relation.oid
                  AND attribute.attnum = key_column.attribute_number
                  AND NOT attribute.attisdropped
                 WHERE relation.oid = {lookup}
                   AND key_column.key_ordinality <= identity_index.indnkeyatts
                 ORDER BY key_column.key_ordinality"
            )
        };
        let rows = if let Some(schema) = relation.schema.as_deref() {
            self.database
                .query(&query(QUALIFIED_LOOKUP), &[&schema, &relation.name])
                .await?
        } else {
            self.database
                .query(&query(UNQUALIFIED_LOOKUP), &[&relation.name])
                .await?
        };
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let relation_oid = first.get::<usize, u32>(0);
        let relation_kind = first.get::<usize, String>(1);
        let relation_wide_identity = first.get::<usize, bool>(6);
        if !RelationKind::from_pg_relkind(&relation_kind)
            .is_some_and(RelationKind::supports_row_identity)
            || !relation_wide_identity
        {
            return Ok(None);
        }

        let primary_key = rows
            .into_iter()
            .map(|row| {
                let selectable = row.get::<usize, bool>(5);
                if !selectable || row.get::<usize, u32>(0) != relation_oid {
                    return None;
                }
                Some(PrimaryKeyColumn {
                    name: row.get(2),
                    attribute_number: row.get(3),
                    type_oid: row.get(4),
                })
            })
            .collect::<Option<Vec<_>>>();
        Ok(primary_key.map(|primary_key| RowIdentityPlan {
            relation_oid,
            primary_key,
        }))
    }

    async fn validated_identity_plan(
        &self,
        table: &QualifiedRelation,
        identity: &RowIdentity,
    ) -> Result<RowIdentityPlan, EngineError> {
        let Some(plan) = self.row_identity_plan(table).await? else {
            return Err(EngineError::StaleRow);
        };
        let matches = plan.relation_oid == identity.relation_oid
            && plan.primary_key.len() == identity.primary_key.len()
            && plan
                .primary_key
                .iter()
                .zip(&identity.primary_key)
                .all(|(column, value)| {
                    column.name == value.column
                        && column.attribute_number == value.attribute_number
                        && column.type_oid == value.type_oid
                });
        if matches {
            Ok(plan)
        } else {
            Err(EngineError::StaleRow)
        }
    }

    fn identity_projection(plan: &RowIdentityPlan) -> String {
        let primary_key = plan
            .primary_key
            .iter()
            .enumerate()
            .map(|(index, column)| {
                format!(
                    "{} AS {}{index}",
                    quote_identifier(&column.name),
                    constants::PRIMARY_KEY_ALIAS_PREFIX
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "tableoid::oid AS {TABLEOID_ALIAS}, ctid::text AS {CTID_ALIAS}, xmin::text AS {XMIN_ALIAS}, {primary_key}"
        )
    }

    fn identity_predicate(
        identity: &RowIdentity,
        relation_name_param: usize,
        relation_oid_param: usize,
        table_oid_param: usize,
        ctid_param: usize,
        xmin_param: usize,
        first_key_param: usize,
    ) -> String {
        let key_predicates = identity
            .primary_key
            .iter()
            .enumerate()
            .map(|(offset, key)| {
                format!(
                    "{} IS NOT DISTINCT FROM ${}",
                    quote_identifier(&key.column),
                    first_key_param + offset
                )
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let key_attributes = identity
            .primary_key
            .iter()
            .map(|key| format!("{}::smallint", key.attribute_number))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "pg_catalog.to_regclass(${relation_name_param}::text)::oid = ${relation_oid_param}::oid
             AND tableoid = ${table_oid_param}::oid
             AND ctid = ${ctid_param}::text::tid
             AND xmin = ${xmin_param}::text::xid
             AND {key_predicates}
             AND EXISTS (
                 SELECT 1
                 FROM pg_catalog.pg_index identity_index
                 WHERE identity_index.indrelid = ${relation_oid_param}::oid
                   AND identity_index.indisprimary
                   AND identity_index.indisvalid
                   AND identity_index.indisready
                   AND identity_index.indnkeyatts = {}
                   AND ARRAY(
                       SELECT key_column.attribute_number
                       FROM pg_catalog.unnest(identity_index.indkey::smallint[])
                           WITH ORDINALITY AS key_column(attribute_number, key_ordinality)
                       WHERE key_column.key_ordinality <= identity_index.indnkeyatts
                       ORDER BY key_column.key_ordinality
                   ) = ARRAY[{key_attributes}]
             )",
            identity.primary_key.len()
        )
    }

    async fn update_cell(
        &self,
        table: QualifiedRelation,
        column: String,
        new_value: Option<String>,
        row_identity: RowIdentity,
        column_type: Option<String>,
    ) -> Result<QueryResult, EngineError> {
        let identity_plan = self.validated_identity_plan(&table, &row_identity).await?;
        let qualified = table.quoted();
        let quoted_column = quote_identifier(&column);
        let cast_expr = column_type
            .as_deref()
            .map(Self::qualify_type)
            .transpose()?
            .map_or_else(|| "$1::text".to_string(), |ty| format!("$1::text::{ty}"));
        let predicate = Self::identity_predicate(&row_identity, 2, 3, 4, 5, 6, 7);
        let projection = Self::identity_projection(&identity_plan);
        let sql = format!(
            "UPDATE {qualified} SET {quoted_column} = {cast_expr} WHERE {predicate} RETURNING {projection}"
        );
        let relation_name = table.quoted();
        let mut params: Vec<&(dyn ToSql + Sync)> = vec![
            &new_value,
            &relation_name,
            &row_identity.relation_oid,
            &row_identity.table_oid,
            &row_identity.ctid,
            &row_identity.xmin,
        ];
        params.extend(
            row_identity
                .primary_key
                .iter()
                .map(|value| value as &(dyn ToSql + Sync)),
        );
        let new_identity = self
            .query_single_identity(&sql, &params, &identity_plan)
            .await?;
        let mut result = QueryResult::empty();
        result.metadata.row_identities = vec![Some(new_identity)];
        Ok(result)
    }

    async fn delete_row(
        &self,
        table: QualifiedRelation,
        row_identity: RowIdentity,
    ) -> Result<QueryResult, EngineError> {
        let identity_plan = self.validated_identity_plan(&table, &row_identity).await?;
        let qualified = table.quoted();
        let predicate = Self::identity_predicate(&row_identity, 1, 2, 3, 4, 5, 6);
        let projection = Self::identity_projection(&identity_plan);
        let sql = format!("DELETE FROM {qualified} WHERE {predicate} RETURNING {projection}");
        let relation_name = table.quoted();
        let mut params: Vec<&(dyn ToSql + Sync)> = vec![
            &relation_name,
            &row_identity.relation_oid,
            &row_identity.table_oid,
            &row_identity.ctid,
            &row_identity.xmin,
        ];
        params.extend(
            row_identity
                .primary_key
                .iter()
                .map(|value| value as &(dyn ToSql + Sync)),
        );
        self.query_single_identity(&sql, &params, &identity_plan)
            .await?;
        Ok(QueryResult::empty())
    }

    async fn refresh_row(
        &self,
        table: QualifiedRelation,
        row_identity: RowIdentity,
    ) -> Result<QueryResult, EngineError> {
        let identity_plan = self.validated_identity_plan(&table, &row_identity).await?;
        let qualified = table.quoted();
        let predicate = Self::identity_predicate(&row_identity, 1, 2, 3, 4, 5, 6);
        let projection = Self::identity_projection(&identity_plan);
        let sql = format!("SELECT *, {projection} FROM {qualified} WHERE {predicate}");
        let relation_name = table.quoted();
        let mut params: Vec<&(dyn ToSql + Sync)> = vec![
            &relation_name,
            &row_identity.relation_oid,
            &row_identity.table_oid,
            &row_identity.ctid,
            &row_identity.xmin,
        ];
        params.extend(
            row_identity
                .primary_key
                .iter()
                .map(|value| value as &(dyn ToSql + Sync)),
        );
        let (column_names, column_types, rows) =
            self.query_with_params_mode(&sql, &params, true).await?;
        if rows.is_empty() {
            return Err(EngineError::StaleRow);
        }
        let mut result = rows_to_result(&column_names, &column_types, &rows, Some(&identity_plan));
        result.metadata.source_table = Some(table);
        result.metadata.source_columns = result.columns.iter().cloned().map(Some).collect();
        Ok(result)
    }

    fn qualify_type(type_name: &str) -> Result<String, EngineError> {
        query_analysis::quote_type_name(type_name)
            .ok_or_else(|| EngineError::InvalidIdentifier(type_name.to_string()))
    }
}

#[async_trait]
impl Engine for DatabaseEngine {
    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        let plan = query_analysis::plan_run_sql(sql)?;
        let identity_plan = if let (Some(relation), Some(_)) =
            (plan.source_table.as_ref(), plan.editable_sql.as_ref())
        {
            self.row_identity_plan(relation).await?
        } else {
            None
        };
        let rewritten = identity_plan.as_ref().and_then(|identity| {
            let columns = identity
                .primary_key
                .iter()
                .map(|column| column.name.clone())
                .collect::<Vec<_>>();
            query_analysis::rewrite_with_primary_key_identity(sql, &columns)
        });
        let identity_plan = rewritten.as_ref().and(identity_plan);
        let query_sql = rewritten.as_deref().unwrap_or(sql);
        let mut result = self.query(query_sql, identity_plan.as_ref(), false).await?;
        result.metadata.source_table.clone_from(&plan.source_table);
        if identity_plan.is_some() {
            result.metadata.source_columns = plan.source_columns.map_or_else(
                || vec![None; result.columns.len()],
                |projection| projection.resolve(&result.columns),
            );
        }
        result.metadata.origin = plan.origin;
        Ok(result)
    }

    async fn crud(&self, action: CrudAction) -> Result<QueryResult> {
        match action {
            CrudAction::SelectTop {
                table,
                limit,
                relation_kind,
            } => {
                let qualified = table.quoted();
                let effective_limit = if limit == 0 { self.page_size } else { limit };
                let identity_plan = if relation_kind.supports_row_identity() {
                    self.row_identity_plan(&table).await?
                } else {
                    None
                };
                let sql = identity_plan.as_ref().map_or_else(
                    || format!("SELECT * FROM {qualified} LIMIT {effective_limit}"),
                    |identity| {
                        let projection = Self::identity_projection(identity);
                        format!("SELECT *, {projection} FROM {qualified} LIMIT {effective_limit}")
                    },
                );
                let mut result = self.query(&sql, identity_plan.as_ref(), true).await?;
                result.metadata.source_table = Some(table.clone());
                if identity_plan.is_some() {
                    result.metadata.source_columns = result
                        .columns
                        .iter()
                        .map(|column| Some(column.clone()))
                        .collect();
                }
                result.metadata.origin = ResultOrigin::SelectTop {
                    table,
                    limit: effective_limit,
                };
                Ok(result)
            }
            CrudAction::UpdateCell {
                table,
                column,
                new_value,
                row_identity,
                column_type,
            } => Ok(self
                .update_cell(table, column, new_value, row_identity, column_type)
                .await?),
            CrudAction::DeleteRow {
                table,
                row_identity,
            } => Ok(self.delete_row(table, row_identity).await?),
            CrudAction::RefreshRow {
                table,
                row_identity,
            } => Ok(self.refresh_row(table, row_identity).await?),
            unsupported => Err(EngineError::UnsupportedCrud(Box::new(unsupported)).into()),
        }
    }

    async fn refresh_catalog(&self) -> Result<CatalogSnapshot> {
        let mut catalog = Catalog::new(self.database.clone());
        catalog.refresh().await?;
        Ok(catalog.snapshot())
    }

    async fn cancel_current(&self) -> Result<bool> {
        Ok(self.database.cancel_current().await?)
    }
}

#[cfg(test)]
mod tests;
