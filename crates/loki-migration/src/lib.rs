pub mod applied_revision;
pub mod migrate_store;
pub mod migration;
#[cfg(feature = "postgres")]
pub mod postgres_revision_storage;
pub mod revision;
pub mod revision_list;

//pub use migration::{migrate_database, reset_database};
use crate::migrate_store::RevisionDatabase;
use crate::postgres_revision_storage::PostgresRevisionStorage;
use bon::bon;
pub use migration::Migration;
pub use revision::Revision;
pub use revision_list::{RevisionList, RevisionStatus};

#[macro_export]
macro_rules! revision {
    ($rev:ident) => {
        Revision::const_new(stringify!($rev), None, None)
    };
    ($rev:literal) => {
        Revision::const_new($rev, None, None)
    };
    ($rev:ident, $apply:literal) => {
        Revision::const_new(stringify!($rev), Some($apply), None)
    };
    ($rev:literal, $apply:literal) => {
        Revision::const_new($rev, Some($apply), None)
    };
    ($rev:ident, $apply:literal, $revert:literal) => {
        Revision::const_new(stringify!($rev), Some($apply), Some($revert))
    };
    ($rev:literal, $apply:literal, $revert:literal) => {
        Revision::const_new($rev, Some($apply), Some($revert))
    };
}

pub struct MigrationBuilder;

#[bon]
impl MigrationBuilder {
    /// Create a migration against a postgres pool with the specified revisions this
    /// requreis the 'postgres' feature
    #[builder(finish_fn = build)]
    #[cfg(feature = "postgres")]
    pub fn postgres(
        revisions: &'static [Revision],
        database_pool: &deadpool_postgres::Pool,
    ) -> Migration<RevisionDatabase<PostgresRevisionStorage>> {
        let storage = PostgresRevisionStorage::new(database_pool);
        let store = RevisionDatabase::builder().storage(storage).build();

        Migration::<RevisionDatabase<PostgresRevisionStorage>>::builder()
            .store(store)
            .revisions(revisions)
            .build()
    }
}
