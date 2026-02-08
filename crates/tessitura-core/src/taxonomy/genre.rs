use serde::{Deserialize, Serialize};

/// A genre classification.
///
/// Genres can be hierarchical (e.g., "Classical > 20th Century").
/// In Phase 2, these will be mapped to LCGFT controlled vocabulary terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Genre {
    /// Display name (e.g., "20th Century Classical").
    pub name: String,

    /// Optional parent genre for hierarchical classification.
    pub parent: Option<String>,

    /// LCGFT URI, if mapped.
    pub lcgft_uri: Option<String>,
}

impl Genre {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: None,
            lcgft_uri: None,
        }
    }

    #[must_use]
    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genre_new() {
        let genre = Genre::new("Classical");
        assert_eq!(genre.name, "Classical");
        assert!(genre.parent.is_none());
    }

    #[test]
    fn test_genre_with_parent() {
        let genre = Genre::new("20th Century").with_parent("Classical");
        assert_eq!(genre.name, "20th Century");
        assert_eq!(genre.parent, Some("Classical".to_string()));
    }
}
