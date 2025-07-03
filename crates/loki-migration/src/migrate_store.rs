use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use bon::Builder;
use mockall::automock;
use tracing::instrument;

use crate::RevisionList;
use crate::{Revision, applied_revision::AppliedRevision};

/// This trait provides the interface we use to handle migrations, the migration logic
/// operates against this trait which abstracts the actual database driver, and allows
/// for better unit testing.
#[allow(async_fn_in_trait)]
#[automock]
pub trait RevisionStore {
    async fn applied_revisions(&self) -> Result<Vec<AppliedRevision>>;
    async fn apply(&self, revisions: &[Revision]) -> Result<()>;
    async fn revert(&self, revisions: &[Revision]) -> Result<()>;
}

/// This trait provides the implementaion of the low level storage interfaces
#[automock(type Row=String;)]
#[allow(async_fn_in_trait)]
pub trait RevisionStorage {
    type Row;

    async fn query_applied(&self) -> Result<Vec<Self::Row>>;
    async fn execute(&self, sql_query: &str) -> Result<()>;
}

/// This is the concrete implementation of the revision storage this handles
/// creating the SQL which is processed using the revision storage trait
#[derive(Clone, Builder)]
pub struct RevisionDatabase<S> {
    #[builder(with = |storage: S| Arc::new(storage))]
    storage: Arc<S>,
}

impl<S> RevisionStore for RevisionDatabase<S>
where
    S: RevisionStorage,
    S::Row: TryInto<AppliedRevision>,
    <S::Row as TryInto<AppliedRevision>>::Error: ToString,
{
    /// Queries the revisions from the specified storage layer and converts them
    /// into a collection of [AppliedRevisions]
    #[instrument(level = "info", skip_all)]
    async fn applied_revisions(&self) -> Result<Vec<AppliedRevision>> {
        let revisions = self
            .storage
            .query_applied()
            .await
            .with_context(|| "failed to query applied revisions")?;

        let revisions = revisions
            .into_iter()
            .map(|row| {
                row.try_into()
                    .map_err(|e| anyhow!("failed to convert row: {}", e.to_string()))
            })
            .collect::<std::result::Result<Vec<AppliedRevision>, _>>()?;

        Ok(revisions)
    }

    #[cfg(feature = "batch-ops")]
    #[instrument(level = "debug", skip_all, fields(revisions = revisions.revision_list()))]
    async fn apply(&self, revisions: &[Revision]) -> Result<()> {
        if revisions.is_empty() {
            use anyhow::Ok;

            return Ok(());
        }

        let mut statements = revisions
            .iter()
            .filter(|revision| revision.has_apply())
            .fold(Vec::new(), |mut statements, revision| {
                statements.push(revision.apply().trim_end_matches(';').trim().to_owned());
                statements
            });

        statements.push(format!(
            "INSERT INTO migrations (rev) VALUES {}",
            revisions
                .iter()
                .fold(Vec::new(), |mut values, revision| {
                    values.push(format!("('{}')", revision.revision()));
                    values
                })
                .join(",")
        ));

        tracing::debug!(
            revisions = revisions.revision_list(),
            statements = ?statements,
            "applying mgration(s)"
        );

        match self.storage.execute(&statements.join("\n;\n")).await {
            Ok(_) => {
                tracing::info!(
                    revisions = revisions.revision_list(),
                    "succesfully applied {} revisions",
                    revisions.len()
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    erorr = e.to_string(),
                    revisions = revisions.revision_list(),
                    statement = ?statements,
                    "failed to apply revision(s) to the database",
                );
                Err(e)
            }
        }
    }

    #[cfg(feature = "batch-ops")]
    #[instrument(level = "debug", skip_all, fields(revisions = revisions.revision_list()))]
    async fn revert(&self, revisions: &[Revision]) -> Result<()> {
        if revisions.is_empty() {
            return Ok(());
        }

        let mut statements = revisions
            .iter()
            .filter(|revision| revision.has_revert())
            .fold(Vec::new(), |mut statements, revision| {
                statements.push(revision.revert().trim_end_matches(';').trim().to_owned());
                statements
            });

        statements.push(format!(
            "DELETE FROM migrations WHERE {}",
            revisions
                .iter()
                .fold(Vec::new(), |mut values, revision| {
                    values.push(format!("rev='{}'", revision.revision()));
                    values
                })
                .join(" OR ")
        ));

        tracing::debug!(
            revisions = revisions.revision_list(),
            statements = ?statements,
            "applying revisions(s)"
        );

        match self.storage.execute(&statements.join("\n;\n")).await {
            Ok(_) => {
                tracing::info!(
                    revisions = revisions.revision_list(),
                    "succesfully reverted {} revisions",
                    revisions.len()
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    erorr = e.to_string(),
                    revisions = revisions.revision_list(),
                    statement = ?statements,
                    "failed to revert revision(s) to the database",
                );
                Err(e)
            }
        }
    }

    #[cfg(not(feature = "batch-ops"))]
    #[instrument(level = "debug", skip_all, fields(revisions = revisions.revision_list()))]
    async fn apply(&self, revisions: &[Revision]) -> Result<()> {
        for revision in revisions.iter() {
            let mut statements = Vec::new();

            if revision.has_apply() {
                statements.push(revision.apply().trim_end_matches(';').trim().to_owned());
            }
            statements.push(format!(
                "INSERT INTO migrations (rev) VALUES ('{}')",
                revision.revision()
            ));

            tracing::debug!(
                revision = revision.revision(),
                statements = ?statements,
                "applying mgration"
            );

            match self.storage.execute(&statements.join("\n;\n")).await {
                Ok(_) => {
                    tracing::info!("succesfully applied {}", revision.revision());
                }
                Err(e) => {
                    tracing::error!(
                        erorr = e.to_string(),
                        revision = revision.revision(),
                        revisions = revisions.revision_list(),
                        statement = ?statements,
                        "failed to apply revision(s) to the database",
                    );
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "batch-ops"))]
    #[instrument(level = "debug", skip_all, fields(revisions = revisions.revision_list()))]

    async fn revert(&self, revisions: &[Revision]) -> Result<()> {
        for revision in revisions.iter() {
            let mut statements = Vec::new();

            if revision.has_revert() {
                statements.push(revision.revert().trim_end_matches(';').trim().to_owned());
            }
            statements.push(format!(
                "DELETE FROM migrations WHERE rev='{}'",
                revision.revision()
            ));

            tracing::debug!(
                revision = revision.revision(),
                statements = ?statements,
                "applying mgration"
            );

            match self.storage.execute(&statements.join("\n;\n")).await {
                Ok(_) => {
                    tracing::info!("succesfully reverted {}", revision.revision());
                }
                Err(e) => {
                    tracing::error!(
                        erorr = e.to_string(),
                        revision = revision.revision(),
                        revisions = revisions.revision_list(),
                        statement = ?statements,
                        "failed to reverrt revision(s) from the database",
                    );
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::revision;

    use super::*;
    use chrono::Utc;
    use mockall::predicate::eq;
    use test_log::test;

    impl TryFrom<String> for AppliedRevision {
        type Error = anyhow::Error;
        fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
            Ok(AppliedRevision::builder()
                .revision(value)
                .timestamp(Utc::now())
                .build())
        }
    }

    #[test(tokio::test)]
    async fn query_applied_one() {
        let mut mock = MockRevisionStorage::new();

        mock.expect_query_applied()
            .once()
            .return_once(|| Ok(vec![String::from("v1_initial")]));

        let db = RevisionDatabase::builder().storage(mock).build();
        let applied = db.applied_revisions().await.unwrap();
        assert_eq!(1, applied.len());
        assert_eq!("v1_initial", applied[0].revision());
    }

    #[test(tokio::test)]
    async fn query_applied() {
        let mut mock = MockRevisionStorage::new();

        mock.expect_query_applied()
            .once()
            .return_once(|| Ok(vec![String::from("v000"), String::from("v001")]));

        let db = RevisionDatabase::builder().storage(mock).build();
        let applied = db.applied_revisions().await.unwrap();
        assert_eq!(2, applied.len());
        assert_eq!("v000", applied[0].revision());
        assert_eq!("v001", applied[1].revision());
    }

    #[test(tokio::test)]
    #[should_panic]
    async fn query_applied_failure() {
        let mut mock = MockRevisionStorage::new();

        mock.expect_query_applied()
            .once()
            .return_once(|| Err(anyhow!("unit test failure")));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.applied_revisions().await.unwrap();
    }

    #[test(tokio::test)]
    async fn apply_one() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [revision!("v000", "SELECT * FROM migrations")];

        mock.expect_execute()
            .with(eq(
                "SELECT * FROM migrations\n;\nINSERT INTO migrations (rev) VALUES ('v000')",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.apply(&revisions).await.unwrap();
    }

    #[test(tokio::test)]
    async fn revert_one() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [revision!(
            "v000",
            "SELECT * FROM migrations",
            "DELETE * FROM migrations"
        )];

        mock.expect_execute()
            .with(eq(
                "DELETE * FROM migrations\n;\nDELETE FROM migrations WHERE rev='v000'",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.revert(&revisions).await.unwrap();
    }

    #[test(tokio::test)]
    #[cfg(feature = "batch-ops")]
    async fn apply_batch() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [
            revision!("v000", "SELECT * FROM migrations"),
            revision!("v001", "CREATE TABLE news   "),
        ];

        mock.expect_execute()
            .with(eq(
                "SELECT * FROM migrations\n;\nCREATE TABLE news\n;\nINSERT INTO migrations (rev) VALUES ('v000'),('v001')",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.apply(&revisions).await.unwrap();
    }

    #[test(tokio::test)]
    #[cfg(feature = "batch-ops")]
    async fn revert_batch() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [
            revision!(
                "v000",
                "SELECT * FROM migrations",
                "DELETE * FROM migrations"
            ),
            revision!("v001", "CREATE TABLE news   ", "DROP TABLE news"),
        ];

        mock.expect_execute()
            .with(eq(
                "DELETE * FROM migrations\n;\nDROP TABLE news\n;\nDELETE FROM migrations WHERE rev='v000' OR rev='v001'",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.revert(&revisions).await.unwrap();
    }

    #[test(tokio::test)]
    #[cfg(not(feature = "batch-ops"))]
    async fn apply_batch() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [
            revision!("v000", "SELECT * FROM migrations"),
            revision!("v001", "CREATE TABLE news   "),
        ];

        mock.expect_execute()
            .once()
            .with(eq(
                "SELECT * FROM migrations\n;\nINSERT INTO migrations (rev) VALUES ('v000')",
            ))
            .return_once(|_| Ok(()));

        mock.expect_execute()
            .once()
            .with(eq(
                "CREATE TABLE news\n;\nINSERT INTO migrations (rev) VALUES ('v001')",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.apply(&revisions).await.unwrap();
    }

    #[test(tokio::test)]
    #[cfg(not(feature = "batch-ops"))]
    async fn revert_batch() {
        let mut mock = MockRevisionStorage::new();
        let revisions = [
            revision!(
                "v000",
                "SELECT * FROM migrations",
                "DELETE * FROM migrations"
            ),
            revision!("v001", "CREATE TABLE news   ", "DROP TABLE news"),
        ];
        mock.expect_execute()
            .once()
            .with(eq(
                "DELETE * FROM migrations\n;\nDELETE FROM migrations WHERE rev='v000'",
            ))
            .return_once(|_| Ok(()));

        mock.expect_execute()
            .once()
            .with(eq(
                "DROP TABLE news\n;\nDELETE FROM migrations WHERE rev='v001'",
            ))
            .return_once(|_| Ok(()));

        let db = RevisionDatabase::builder().storage(mock).build();
        db.revert(&revisions).await.unwrap();
    }
}
