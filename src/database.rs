use std::str::FromStr;

use crate::config::Config;
use crate::migrations::DATABASE_REVISIONS;
use anyhow::{Context, Result};
use deadpool_postgres::{Manager, Pool};
use loki_migration::MigrationBuilder;
use tokio_postgres::NoTls;
use tracing::instrument;

#[instrument(level = "info", skip_all)]
pub async fn initialize_database(config: &Config) -> Result<Pool> {
    let database_config = tokio_postgres::Config::from_str(&config.database_url)?;
    let pool = Pool::builder(Manager::new(database_config, NoTls))
        .build()
        .with_context(|| "failed to create the database pool")?;
    exeute_migrations(&config, &pool).await?;

    tracing::debug!("connected to database");
    Ok(pool)
}

async fn exeute_migrations(config: &Config, database_pool: &Pool) -> Result<()> {
    let migration = MigrationBuilder::postgres()
        .database_pool(database_pool)
        .revisions(DATABASE_REVISIONS)
        .build();

    if config.reset_datbase {
        migration
            .reset()
            .await
            .with_context(|| "failed to reset the database")?;
    } else if let Some(revisions) = config.revert_database {
        migration
            .downgrade(Some(revisions))
            .await
            .with_context(|| format!("failed to apply downgrades [{revisions}"))?;
    } else {
        migration
            .upgrade()
            .await
            .with_context(|| "failed to execute upgrade")?;
    }

    Ok(())
}
