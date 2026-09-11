use super::Database;
use crate::error::DatabaseError;
use futures::TryStreamExt;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_postgres::types::ToSql;

impl Database {
    /// Streams the results of a `COPY TO STDOUT` SQL statement into an in-memory buffer.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if parameters are supplied, the stream cannot be opened,
    /// or chunks cannot be read.
    pub async fn copy_out_csv(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<u8>, DatabaseError> {
        if !params.is_empty() {
            return Err(DatabaseError::CopyOutParametersUnsupported);
        }

        Self::assert_copy_out_params(params)?;
        let mut buffer = Vec::new();
        self.stream_copy_out(sql, |chunk| {
            buffer.extend_from_slice(chunk);
            Ok(())
        })
        .await?;
        Ok(buffer)
    }

    /// Streams the results of a `COPY TO STDOUT` SQL statement into the provided writer.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if parameters are supplied, the stream cannot be opened,
    /// reading or writing the chunks fails, or the writer cannot be flushed.
    pub async fn copy_out_csv_into<W>(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        mut writer: std::pin::Pin<&mut W>,
    ) -> Result<u64, DatabaseError>
    where
        W: AsyncWrite + ?Sized,
    {
        Self::assert_copy_out_params(params)?;

        let client = self.acquire().await?;
        let stream = client
            .copy_out(sql)
            .await
            .map_err(DatabaseError::Postgres)?;
        tokio::pin!(stream);
        let mut bytes_copied = 0u64;
        while let Some(chunk) = stream
            .as_mut()
            .try_next()
            .await
            .map_err(DatabaseError::Postgres)?
        {
            writer
                .as_mut()
                .write_all(&chunk)
                .await
                .map_err(DatabaseError::CopyOutIo)?;
            bytes_copied += chunk.len() as u64;
        }
        writer
            .as_mut()
            .flush()
            .await
            .map_err(DatabaseError::CopyOutIo)?;
        Ok(bytes_copied)
    }

    /// Copy chunks from `COPY ... TO STDOUT` into a synchronous consumer and return the byte count.
    async fn stream_copy_out<F>(&self, sql: &str, mut on_chunk: F) -> Result<u64, DatabaseError>
    where
        F: FnMut(&[u8]) -> Result<(), DatabaseError>,
    {
        let client = self.acquire().await?;
        let stream = client
            .copy_out(sql)
            .await
            .map_err(DatabaseError::Postgres)?;
        tokio::pin!(stream);
        let mut bytes_copied = 0u64;
        while let Some(chunk) = stream
            .as_mut()
            .try_next()
            .await
            .map_err(DatabaseError::Postgres)?
        {
            on_chunk(&chunk)?;
            bytes_copied += chunk.len() as u64;
        }
        Ok(bytes_copied)
    }

    fn assert_copy_out_params(params: &[&(dyn ToSql + Sync)]) -> Result<(), DatabaseError> {
        if params.is_empty() {
            return Ok(());
        }
        Err(DatabaseError::CopyOutParametersUnsupported)
    }
}
