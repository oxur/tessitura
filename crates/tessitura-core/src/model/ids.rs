use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<Uuid> for $name {
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }
    };
}

define_id!(WorkId, "Unique identifier for a musical work.");
define_id!(
    ExpressionId,
    "Unique identifier for a performance/recording."
);
define_id!(ManifestationId, "Unique identifier for a release.");
define_id!(
    ItemId,
    "Unique identifier for a physical/digital file."
);
define_id!(ArtistId, "Unique identifier for an artist.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_id_generation() {
        let id1 = WorkId::new();
        let id2 = WorkId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_work_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let id = WorkId::from_uuid(uuid);
        assert_eq!(*id.as_uuid(), uuid);
    }

    #[test]
    fn test_work_id_display() {
        let id = WorkId::new();
        let display = id.to_string();
        assert!(!display.is_empty());
    }

    #[test]
    fn test_id_types_are_distinct() {
        let work_uuid = Uuid::new_v4();
        let expr_uuid = Uuid::new_v4();

        let _work_id = WorkId::from_uuid(work_uuid);
        let _expr_id = ExpressionId::from_uuid(expr_uuid);

        // Type system ensures we can't mix these
    }
}
