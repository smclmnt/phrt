use anyhow::{Context, Result, anyhow};
use bon::bon;
use tracing::instrument;

use super::revision_list::{RevisionList, RevisionStatus};
use crate::Revision;
use crate::migrate_store::RevisionStore;

pub struct Migration<S> {
    store: S,
    revisions: &'static [Revision],
}

#[bon]
impl<S> Migration<S>
where
    S: RevisionStore,
{
    #[builder]
    pub(crate) fn new(store: S, revisions: &'static [Revision]) -> Self {
        Self { store, revisions }
    }

    #[instrument(level = "info", skip_all, fields(revisions = self.revisions.revision_list()))]
    pub async fn needs_migration(&self) -> bool {
        let applied_revisions = self.store.applied_revisions().await.unwrap_or_default();
        self.revisions.iter().all(|revision| {
            applied_revisions
                .iter()
                .find(|appled_revision| appled_revision.revision() == revision.revision())
                .is_some()
        })
    }

    #[instrument(level = "info", skip_all, fields(revisions = self.revisions.revision_list()))]
    pub async fn upgrade(&self) -> Result<usize> {
        let applied_revisions = self
            .store
            .applied_revisions()
            .await
            .with_context(|| "failed to retrieve applied revisions")?;

        let (to_apply, _already_applied): (Vec<Revision>, Vec<Revision>) =
            self.revisions.iter().cloned().partition(|revision| {
                applied_revisions
                    .iter()
                    .find(|applied| applied.revision() == revision.revision())
                    .is_none()
            });

        if to_apply.is_empty() {
            tracing::info!("database has all required revisions");
            return Ok(0);
        }

        tracing::debug!(
            "preparing to apply migration applied={}, needed={}",
            applied_revisions.revision_status(),
            to_apply.revision_list(),
        );

        match self.store.apply(&to_apply).await {
            Ok(_) => {
                tracing::info!(
                    revisions = to_apply.revision_list(),
                    "successfully applied {} revision(s)",
                    to_apply.len()
                );
                Ok(to_apply.len())
            }
            Err(e) => {
                tracing::error!(
                    "failed to apply migrations {:?}: {e}",
                    to_apply.revision_list()
                );
                Err(anyhow!("failed to apply migrations: {e}"))
            }
        }
    }

    #[instrument(level = "info", skip_all, fields(revisions = self.revisions.revision_list()))]
    pub async fn downgrade(&self, revisions: Option<usize>) -> Result<usize> {
        let applied = self
            .store
            .applied_revisions()
            .await
            .with_context(|| "failed to retrieve the applied versions")?;

        let applied = self
            .revisions
            .iter()
            .filter_map(|revision| {
                applied
                    .iter()
                    .find(|applied_revision| applied_revision.revision() == revision.revision())
                    .map(|_| revision)
            })
            .cloned()
            .collect::<Vec<Revision>>();

        let (to_revert, applied) = if let Some(count) = revisions {
            let start = (applied.len() - count).max(0);
            (&applied[start..], &applied[..start])
        } else {
            (applied.as_slice(), &[] as &[Revision])
        };

        if to_revert.is_empty() {
            tracing::info!("no revisions to revert {revisions:?}");
            return Ok(0);
        }

        tracing::debug!("preparing to revert {} migrations", to_revert.len(),);

        match self
            .store
            .revert(&to_revert.iter().rev().cloned().collect::<Vec<Revision>>())
            .await
        {
            Ok(_) => {
                tracing::info!(
                    revisions = to_revert.revision_list(),
                    appllied = applied.revision_list(),
                    "reverted {} revisions",
                    to_revert.len()
                );
                Ok(to_revert.len())
            }
            Err(e) => {
                tracing::error!(
                    revisions = to_revert.revision_list(),
                    "failed to revert {} revisions",
                    to_revert.len()
                );
                Err(anyhow!(
                    "failed to revert {}: {e}",
                    to_revert.revision_list()
                ))
            }
        }
    }

    #[instrument(level = "info", skip_all, fields(revisions = self.revisions.revision_list()))]
    pub async fn reset(&self) -> Result<()> {
        let applied = self
            .store
            .applied_revisions()
            .await
            .with_context(|| "failed to retrieve the applied versions")?;

        let revert = self
            .revisions
            .iter()
            .filter(|revision| applied.contains_revision(revision.revision()))
            .cloned()
            .rev()
            .collect::<Vec<Revision>>();

        match self.store.revert(&revert).await {
            Ok(_) => {
                tracing::info!(
                    revert = revert.revision_list(),
                    "succesfully reverted {} revision(s)",
                    revert.len()
                );
            }
            Err(e) => {
                tracing::error!(
                    revisions = revert.revision_list(),
                    ertror = e.to_string(),
                    "failed to revert {} migration(s) during reset",
                    revert.len()
                );
                return Err(e);
            }
        }

        match self.store.apply(self.revisions).await {
            Ok(_) => {
                tracing::info!(
                    revisions = self.revisions.revision_list(),
                    "succesfully applied {} revision(s)",
                    self.revisions.len(),
                );
            }
            Err(e) => {
                tracing::error!(
                    error = e.to_string(),
                    revisions = self.revisions.revision_list(),
                    "failed to apply {} revision(s) during reset",
                    self.revisions.len()
                );
                return Err(e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::applied_revision::AppliedRevision;
    use crate::migrate_store::MockRevisionStore;
    use crate::revision;
    use chrono::Utc;
    use mockall::predicate::*;
    use test_log::test;

    macro_rules! applied_revision {
        ($rev:literal) => {
            AppliedRevision::new($rev, Utc::now())
        };
        ($rev:ident) => {
            AppliedRevision::new(stringify!($eev), DateTime::now())
        };
    }

    #[test(tokio::test)]
    async fn full_migration() {
        static REVS: [Revision; 2] = [revision!("1"), revision!("2")];
        let mut mock = MockRevisionStore::new();

        mock.expect_applied_revisions()
            .once()
            .returning(|| Ok(Default::default()));

        mock.expect_apply()
            .with(eq(REVS.to_vec()))
            .returning(|_| Ok(()));

        let migration = Migration::builder()
            .store(mock)
            .revisions(REVS.as_slice())
            .build();

        assert_eq!(REVS.len(), migration.upgrade().await.unwrap());
    }

    #[test(tokio::test)]
    async fn require_one_migration() {
        static REVS: [Revision; 3] = [revision!("1"), revision!("2"), revision!("3")];

        let mut mock = MockRevisionStore::new();
        mock.expect_applied_revisions()
            .once()
            .returning(|| Ok(vec![applied_revision!("2"), applied_revision!("1")]));

        mock.expect_apply()
            .with(eq([revision!("3")]))
            .returning(|_| Ok(()));

        let migration = Migration::builder()
            .store(mock)
            .revisions(REVS.as_slice())
            .build();

        assert_eq!(1, migration.upgrade().await.unwrap());
    }

    #[test(tokio::test)]
    async fn revert_all() {
        static REVS: [Revision; 3] = [revision!("1"), revision!("2"), revision!("3")];

        let mut mock = MockRevisionStore::new();
        mock.expect_applied_revisions().once().returning(|| {
            Ok(vec![
                applied_revision!("2"),
                applied_revision!("1"),
                applied_revision!("3"),
            ])
        });

        mock.expect_revert()
            .once()
            .with(eq(REVS.iter().cloned().rev().collect::<Vec<Revision>>()))
            .returning(|_| Ok(()));

        let migration = Migration::builder()
            .store(mock)
            .revisions(REVS.as_slice())
            .build();

        assert_eq!(3, migration.downgrade(None).await.unwrap());
    }

    #[test(tokio::test)]
    async fn revert_single() {
        static REVS: [Revision; 3] = [revision!("1"), revision!("2"), revision!("3")];
        let mut mock = MockRevisionStore::new();

        mock.expect_applied_revisions()
            .once()
            .returning(|| Ok(vec![applied_revision!("2"), applied_revision!("1")]));

        mock.expect_revert()
            .once()
            .with(eq(vec![revision!("2")]))
            .returning(|_| Ok(()));

        let migration = Migration::builder()
            .store(mock)
            .revisions(REVS.as_slice())
            .build();

        assert_eq!(1, migration.downgrade(Some(1)).await.unwrap());
    }

    #[test(tokio::test)]
    async fn reset() {
        static REVS: [Revision; 3] = [revision!("1"), revision!("2"), revision!("3")];

        let mut mock = MockRevisionStore::new();
        mock.expect_applied_revisions()
            .returning(|| Ok(vec![applied_revision!("2"), applied_revision!("1")]));

        mock.expect_revert()
            .once()
            .with(eq(REVS[0..=1]
                .iter()
                .cloned()
                .rev()
                .collect::<Vec<Revision>>()))
            .returning(|_| Ok(()));

        mock.expect_apply()
            .once()
            .with(eq(REVS.to_vec()))
            .returning(|_| Ok(()));

        let migration = Migration::builder()
            .store(mock)
            .revisions(REVS.as_slice())
            .build();

        migration.reset().await.unwrap();
    }
}
