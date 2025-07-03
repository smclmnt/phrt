/// Structrure to represent a migration to the specified database version
#[derive(Clone, Eq, PartialEq)]
pub struct Revision {
    /// The revision represented by this entry
    pub revision: &'static str,
    /// The SQL to perform the apply this revison
    pub apply: Option<&'static str>,
    /// the SQL to peform the revert this revision (can be empty)
    pub revert: Option<&'static str>,
}

impl Revision {
    pub const fn const_new(
        revision: &'static str,
        apply: Option<&'static str>,
        revert: Option<&'static str>,
    ) -> Self {
        Self {
            revision,
            apply,
            revert,
        }
    }
    pub fn apply(&self) -> &str {
        self.apply.unwrap_or_default()
    }

    pub fn revert(&self) -> &str {
        self.revert.unwrap_or_default()
    }

    pub fn revision(&self) -> &str {
        self.revision
    }

    pub fn has_apply(&self) -> bool {
        self.apply.is_some_and(|a| !a.is_empty())
    }

    pub fn has_revert(&self) -> bool {
        self.revert.is_some_and(|r| !r.is_empty())
    }
}

impl std::fmt::Debug for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut struct_writer = f.debug_struct("Revision");
        struct_writer.field("revision", &self.revision);
        if !self.has_apply() {
            struct_writer.field("apply", &self.apply());
        }
        if !self.has_revert() {
            struct_writer.field("revert", &self.revert());
        }
        struct_writer.finish()
    }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "revision({})", self.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revision;

    #[test]
    fn delcare_empty_revision_ident() {
        let revision = revision!(v1);
        assert_eq!("v1", revision.revision());
        assert!(!revision.has_apply());
        assert!(!revision.has_revert());
    }

    #[test]
    fn delcare_empty_revision_literal() {
        let revision = revision!("1");
        assert_eq!("1", revision.revision());
        assert!(!revision.has_apply());
        assert!(!revision.has_revert());
    }

    #[test]
    fn declare_apply_revision() {
        let revision = revision!(v2, "SELECT");
        assert_eq!("v2", revision.revision());
        assert_eq!("SELECT", revision.apply());
        assert!(!revision.has_revert());
    }

    #[test]
    fn declare_apply_revision_literal() {
        let revision = revision!("2", "SELECT");
        assert_eq!("2", revision.revision());
        assert_eq!("SELECT", revision.apply());
        assert!(!revision.has_revert());
    }

    #[test]
    fn declare_full_revision() {
        let revision = revision!(v_1, "SELECT", "DROP TABLE IF EXISTS fred");
        assert_eq!("v_1", revision.revision());
        assert_eq!("SELECT", revision.apply());
        assert_eq!("DROP TABLE IF EXISTS fred", revision.revert());
    }

    #[test]
    fn declare_full_revision_literal() {
        let revision = revision!("6", "SELECT", "DROP TABLE IF EXISTS fred");
        assert_eq!("6", revision.revision());
        assert_eq!("SELECT", revision.apply());
        assert_eq!("DROP TABLE IF EXISTS fred", revision.revert());
    }
}
