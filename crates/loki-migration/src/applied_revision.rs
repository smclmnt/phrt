use bon::Builder;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Eq, PartialEq, Builder)]
pub struct AppliedRevision {
    revision: String,
    timestamp: DateTime<Utc>,
}

impl AppliedRevision {
    #[cfg(test)]
    pub(crate) fn new(revision: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self::builder()
            .revision(revision.into())
            .timestamp(timestamp)
            .build()
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn applied_at(&self) -> &DateTime<Utc> {
        &self.timestamp
    }
}
