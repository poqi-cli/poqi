use crate::models::RowIdentityPlan;
use crate::value_format::{decode_row_identity, format_type_name};
use crate::{DatabaseEngine, EngineError, RowIdentity};
use tokio_postgres::{types::ToSql, Row};

#[derive(Clone, Debug)]
pub(super) enum TransactionTask {
    Query { read_only: bool },
    QuerySingleIdentity(RowIdentityPlan),
}

pub(super) enum TransactionOutcome {
    QueryResult {
        column_names: Vec<String>,
        column_types: Vec<String>,
        rows: Vec<Row>,
    },
    RowIdentity(Option<RowIdentity>),
}

impl DatabaseEngine {
    pub(super) fn statement_timeout_ms(&self) -> i64 {
        let timeout_ms_u128 = self.statement_timeout.as_millis().min(i64::MAX as u128);
        i64::try_from(timeout_ms_u128).unwrap_or(i64::MAX)
    }

    pub(super) async fn query_with_params_mode(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        read_only: bool,
    ) -> Result<(Vec<String>, Vec<String>, Vec<Row>), EngineError> {
        match self
            .run_transaction(sql, params, TransactionTask::Query { read_only })
            .await?
        {
            TransactionOutcome::QueryResult {
                column_names,
                column_types,
                rows,
            } => Ok((column_names, column_types, rows)),
            TransactionOutcome::RowIdentity(_) => {
                unreachable!("query transaction should return row data")
            }
        }
    }

    pub(super) async fn query_single_identity(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        identity_plan: &RowIdentityPlan,
    ) -> Result<RowIdentity, EngineError> {
        match self
            .run_transaction(
                sql,
                params,
                TransactionTask::QuerySingleIdentity(identity_plan.clone()),
            )
            .await?
        {
            TransactionOutcome::RowIdentity(Some(value)) => Ok(value),
            TransactionOutcome::RowIdentity(None) => Err(EngineError::StaleRow),
            TransactionOutcome::QueryResult { .. } => {
                unreachable!("identity transaction should return row identity")
            }
        }
    }

    pub(super) async fn run_transaction(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        task: TransactionTask,
    ) -> Result<TransactionOutcome, EngineError> {
        let timeout_ms = self.statement_timeout_ms();
        let sql_owned = sql.to_owned();
        let params = params.to_vec();
        let outcome = self
            .database
            .with_client(move |mut client| {
                let sql = sql_owned.clone();
                let params = params.clone();
                let task = task;
                async move {
                    let transaction = client.transaction().await?;
                    if matches!(&task, TransactionTask::Query { read_only: true }) {
                        transaction
                            .batch_execute("SET TRANSACTION READ ONLY")
                            .await?;
                    }
                    transaction
                        .batch_execute(&format!("SET LOCAL statement_timeout = {timeout_ms}"))
                        .await?;
                    let statement = transaction.prepare(&sql).await?;
                    let outcome = match task {
                        TransactionTask::Query { .. } => {
                            let columns_meta = statement.columns();
                            let column_names = columns_meta
                                .iter()
                                .map(|col| col.name().to_string())
                                .collect::<Vec<_>>();
                            let column_types = columns_meta
                                .iter()
                                .map(|col| format_type_name(col.type_()))
                                .collect::<Vec<_>>();
                            let rows = transaction.query(&statement, &params).await?;
                            TransactionOutcome::QueryResult {
                                column_names,
                                column_types,
                                rows,
                            }
                        }
                        TransactionTask::QuerySingleIdentity(identity_plan) => {
                            let identity = transaction
                                .query_opt(&statement, &params)
                                .await?
                                .and_then(|row| {
                                    decode_row_identity(&row, Some(&identity_plan), Some(0))
                                });
                            TransactionOutcome::RowIdentity(identity)
                        }
                    };
                    transaction.commit().await?;
                    Ok::<_, tokio_postgres::Error>(outcome)
                }
            })
            .await?;
        Ok(outcome)
    }
}
