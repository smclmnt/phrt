use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use tokio::sync::OnceCell;
use tokio_postgres::Row;
use tracing::instrument;

use crate::applied_revision::AppliedRevision;
use crate::migrate_store::RevisionStorage;

pub struct PostgresRevisionStorage {
    pool: Pool,
    ensure_table: OnceCell<Result<()>>,
}

impl PostgresRevisionStorage {
    pub fn new(pool: &Pool) -> Self {
        Self {
            pool: pool.clone(),
            ensure_table: OnceCell::new(),
        }
    }

    /// Ensure we have a migrations table in the database
    async fn ensure_migrations_table(&self) -> Result<()> {
        self.ensure_table
            .get_or_init(|| async move {
                self.pool
                    .get()
                    .await?
                    .execute(
                        r#"
                        CREATE TABLE IF NOT EXISTS migrations (
                            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                            rev TEXT NOT NULL UNIQUE,
                            timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
                        )
                        "#,
                        &[],
                    )
                    .await
                    .with_context(|| "failed to ensure the migration table")?;
                Ok(())
            })
            .await
            .as_ref()
            .map(|_| ())
            .map_err(|e| anyhow!("failed to ensure 'migrations': {e}"))
    }
}

impl RevisionStorage for PostgresRevisionStorage {
    type Row = tokio_postgres::Row;

    #[instrument(level = "debug", skip_all)]
    async fn query_applied(&self) -> Result<Vec<Self::Row>> {
        self.ensure_migrations_table().await?;

        self.pool
            .get()
            .await?
            .query("SELECT rev,timestamp FROM migrations", &[])
            .await
            .map_err(|e| anyhow!("failed to query for applied revisions {e:?}"))
    }

    #[instrument(level = "debug", skip_all)]
    async fn execute(&self, sql_query: &str) -> Result<()> {
        self.ensure_migrations_table().await?;

        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;

        transaction
            .batch_execute(sql_query)
            .await
            .with_context(|| format!("failed to execute statements"))?;

        transaction
            .commit()
            .await
            .with_context(|| "failed to commit transaction")?;

        Ok(())
    }
}

impl TryFrom<Row> for AppliedRevision {
    type Error = anyhow::Error;

    fn try_from(value: Row) -> std::result::Result<Self, Self::Error> {
        let revision: String = value.try_get(0)?;
        let timestamp: DateTime<Utc> = value.try_get(1)?;

        Ok(AppliedRevision::builder()
            .revision(revision)
            .timestamp(timestamp)
            .build())
    }
}
