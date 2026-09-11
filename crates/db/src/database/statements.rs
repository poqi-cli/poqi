use super::Database;
use crate::error::DatabaseError;
use tokio_postgres::{types::ToSql, Row};

impl Database {
    /// Executes raw SQL without returning rows using the shared connection pool.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if acquiring a client or executing the SQL fails.
    pub async fn batch_execute(&self, sql: &str) -> Result<(), DatabaseError> {
        self.with_client(|client| async move {
            client.batch_execute(sql).await?;
            Ok(())
        })
        .await
    }

    /// Executes a parameterized statement and returns the number of affected rows.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if preparing the statement or executing it fails.
    pub async fn execute(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, DatabaseError> {
        match self
            .with_prepared_statement(sql, params, StatementOperation::Execute)
            .await?
        {
            StatementOutcome::Affected(rows) => Ok(rows),
            _ => unreachable!("execute operation should return affected row count"),
        }
    }

    /// Executes a parameterized query and returns the resulting rows.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if preparing the statement or executing it fails.
    pub async fn query(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, DatabaseError> {
        match self
            .with_prepared_statement(sql, params, StatementOperation::Query)
            .await?
        {
            StatementOutcome::Rows(rows) => Ok(rows),
            _ => unreachable!("query operation should return row set"),
        }
    }

    /// Executes a parameterized query and returns exactly one row.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if preparing the statement fails or the query does not
    /// complete successfully.
    pub async fn query_one(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, DatabaseError> {
        match self
            .with_prepared_statement(sql, params, StatementOperation::QueryOne)
            .await?
        {
            StatementOutcome::Row(row) => Ok(row),
            _ => unreachable!("query_one operation should return a single row"),
        }
    }

    /// Executes a parameterized query and returns at most one row.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if preparing the statement fails or the query cannot be
    /// executed.
    pub async fn query_opt(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, DatabaseError> {
        match self
            .with_prepared_statement(sql, params, StatementOperation::QueryOpt)
            .await?
        {
            StatementOutcome::OptionalRow(row) => Ok(row),
            _ => unreachable!("query_opt operation should return an optional row"),
        }
    }

    /// Prepare (or reuse) a statement on a pooled client and execute the requested operation.
    async fn with_prepared_statement(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        operation: StatementOperation,
    ) -> Result<StatementOutcome, DatabaseError> {
        let sql = sql.to_owned();
        self.with_client(move |client| {
            let sql = sql.clone();
            async move {
                let statement = client.prepare_cached(&sql).await?;
                let outcome = match operation {
                    StatementOperation::Execute => {
                        StatementOutcome::Affected(client.execute(&statement, params).await?)
                    }
                    StatementOperation::Query => {
                        StatementOutcome::Rows(client.query(&statement, params).await?)
                    }
                    StatementOperation::QueryOne => {
                        StatementOutcome::Row(client.query_one(&statement, params).await?)
                    }
                    StatementOperation::QueryOpt => {
                        StatementOutcome::OptionalRow(client.query_opt(&statement, params).await?)
                    }
                };
                Ok(outcome)
            }
        })
        .await
    }
}

#[derive(Clone, Copy, Debug)]
enum StatementOperation {
    Execute,
    Query,
    QueryOne,
    QueryOpt,
}

enum StatementOutcome {
    Affected(u64),
    Rows(Vec<Row>),
    Row(Row),
    OptionalRow(Option<Row>),
}
