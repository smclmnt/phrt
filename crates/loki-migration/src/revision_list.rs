use super::Revision;
use crate::applied_revision::AppliedRevision;

/// A simple trait the decliares a method to format a revision list, works with
/// both [Revision] and [AppliedRevision] structs.
pub trait RevisionList {
    fn revision_list(&self) -> String;
    fn contains_revision(&self, revision: &str) -> bool;
}

pub trait RevisionStatus {
    fn revision_status(&self) -> String;
}

impl RevisionList for Vec<Revision> {
    #[inline(always)]
    fn revision_list(&self) -> String {
        self.as_slice().revision_list()
    }

    #[inline(always)]
    fn contains_revision(&self, revision: &str) -> bool {
        self.as_slice().contains_revision(revision)
    }
}

impl RevisionList for &[Revision] {
    fn revision_list(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(Revision::revision)
                .map(|revision| revision.replace([' ', '\t', '\r'], "_"))
                .collect::<Vec<String>>()
                .join(";")
        )
    }

    fn contains_revision(&self, revision: &str) -> bool {
        self.iter().any(|rev| rev.revision() == revision)
    }
}

impl RevisionList for Vec<AppliedRevision> {
    #[inline(always)]
    fn revision_list(&self) -> String {
        self.as_slice().revision_list()
    }

    #[inline(always)]
    fn contains_revision(&self, revision: &str) -> bool {
        self.as_slice().contains_revision(revision)
    }
}

impl RevisionList for &[AppliedRevision] {
    fn revision_list(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(AppliedRevision::revision)
                .map(|revision| revision.replace([' ', '\t', '\r'], "_"))
                .collect::<Vec<String>>()
                .join(";")
        )
    }

    fn contains_revision(&self, revision: &str) -> bool {
        self.iter().any(|rev| rev.revision() == revision)
    }
}

impl RevisionStatus for Vec<AppliedRevision> {
    #[inline(always)]
    fn revision_status(&self) -> String {
        self.as_slice().revision_status()
    }
}

impl RevisionStatus for &[AppliedRevision] {
    fn revision_status(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(AppliedRevision::revision_status)
                .collect::<Vec<String>>()
                .join(";")
        )
    }
}

impl RevisionStatus for AppliedRevision {
    fn revision_status(&self) -> String {
        format!(
            "{}@{}",
            self.revision().replace([' ', '\n', '\r'], "_"),
            self.applied_at().to_rfc3339()
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::revision;
    use chrono::Utc;

    use super::*;

    #[test]
    fn empty_revision_vec() {
        let revisions = Vec::<Revision>::new();
        assert_eq!("[]", revisions.revision_list());
    }

    #[test]
    fn empty_applied_revision_vec() {
        let revisions = Vec::<AppliedRevision>::new();
        assert_eq!("[]", revisions.revision_list());
    }

    #[test]
    fn sinle_revision_vec() {
        let revisions = vec![revision!(v00)];
        assert_eq!("[v00]", revisions.revision_list());
    }

    #[test]
    fn sinle_applied_revision_vec() {
        let revisions = vec![
            AppliedRevision::builder()
                .revision(String::from("0000_create_news_table"))
                .timestamp(Utc::now())
                .build(),
        ];
        assert_eq!("[0000_create_news_table]", revisions.revision_list());
    }

    #[test]
    fn single_revision_vec_spaces() {
        let revisions = vec![revision!("v 00")];
        assert_eq!("[v_00]", revisions.revision_list());
    }

    #[test]
    fn single_applied_revision_vec_spaces() {
        let revisions = vec![
            AppliedRevision::builder()
                .revision(String::from("v 00 create news"))
                .timestamp(Utc::now())
                .build(),
        ];
        assert_eq!("[v_00_create_news]", revisions.revision_list());
    }

    #[test]
    fn revisions_vec() {
        let revisions = vec![revision!(v00), revision!(v01_create)];
        assert_eq!("[v00;v01_create]", revisions.revision_list());
    }

    #[test]
    fn applied_revisions_vec() {
        let revisions = vec![
            AppliedRevision::builder()
                .revision(String::from("v00"))
                .timestamp(Utc::now())
                .build(),
            AppliedRevision::builder()
                .revision(String::from("v01_initial"))
                .timestamp(Utc::now())
                .build(),
        ];
        assert_eq!("[v00;v01_initial]", revisions.revision_list());
    }

    #[test]
    fn applied_revisions_vec_status() {
        let now = Utc::now();
        let timestamp = now.to_rfc3339();

        let revisions = vec![
            AppliedRevision::builder()
                .revision(String::from("v00"))
                .timestamp(now)
                .build(),
            AppliedRevision::builder()
                .revision(String::from("v01_initial"))
                .timestamp(now)
                .build(),
        ];
        assert_eq!(
            format!("[v00@{timestamp};v01_initial@{timestamp}]"),
            revisions.revision_status(),
        );
    }

    #[test]
    fn applied_revision_status() {
        let now = Utc::now();
        let timestamp = now.to_rfc3339();

        let revision = AppliedRevision::builder()
            .revision(String::from("v 00"))
            .timestamp(now)
            .build();

        assert_eq!(format!("v_00@{timestamp}"), revision.revision_status(),);
    }
}
