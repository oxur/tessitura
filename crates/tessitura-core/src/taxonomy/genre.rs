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

/// A Library of Congress Genre/Form Term (LCGFT).
///
/// LCGFT provides standardized genre/form vocabulary for classifying
/// creative works. Terms form a hierarchy via `broader_uri`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcgftTerm {
    /// The canonical URI (e.g., "http://id.loc.gov/authorities/genreForms/gf2014026639").
    pub uri: String,
    /// The preferred label (e.g., "String quartets").
    pub label: String,
    /// URI of the broader (parent) term, if any.
    pub broader_uri: Option<String>,
    /// Scope note explaining usage of this term.
    pub scope_note: Option<String>,
}

impl LcgftTerm {
    #[must_use]
    pub fn new(uri: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            label: label.into(),
            broader_uri: None,
            scope_note: None,
        }
    }

    #[must_use]
    pub fn with_broader(mut self, broader_uri: impl Into<String>) -> Self {
        self.broader_uri = Some(broader_uri.into());
        self
    }

    #[must_use]
    pub fn with_scope_note(mut self, note: impl Into<String>) -> Self {
        self.scope_note = Some(note.into());
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

    #[test]
    fn test_lcgft_term_new() {
        let term = LcgftTerm::new(
            "http://id.loc.gov/authorities/genreForms/gf2014026639",
            "String quartets",
        );
        assert_eq!(term.label, "String quartets");
        assert!(term.broader_uri.is_none());
        assert!(term.scope_note.is_none());
    }

    #[test]
    fn test_lcgft_term_with_broader() {
        let term = LcgftTerm::new(
            "http://id.loc.gov/authorities/genreForms/gf2014026639",
            "String quartets",
        )
        .with_broader("http://id.loc.gov/authorities/genreForms/gf2014026090")
        .with_scope_note("Chamber music for two violins, viola, and cello");
        assert_eq!(
            term.broader_uri,
            Some("http://id.loc.gov/authorities/genreForms/gf2014026090".to_string())
        );
        assert!(term.scope_note.is_some());
    }
}
