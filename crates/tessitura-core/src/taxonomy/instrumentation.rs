use serde::{Deserialize, Serialize};

/// An instrument or medium of performance.
///
/// In Phase 2, these will be mapped to LCMPT controlled vocabulary terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub abbreviation: Option<String>,
    pub lcmpt_uri: Option<String>,
}

impl Instrument {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            abbreviation: None,
            lcmpt_uri: None,
        }
    }
}

/// A Library of Congress Medium of Performance Term (LCMPT).
///
/// LCMPT provides standardized vocabulary for describing the
/// instruments and voices used in musical performances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcmptTerm {
    /// The canonical URI (e.g., "http://id.loc.gov/authorities/performanceMediums/mp2013015550").
    pub uri: String,
    /// The preferred label (e.g., "violin").
    pub label: String,
    /// URI of the broader (parent) term, if any.
    pub broader_uri: Option<String>,
    /// Scope note explaining usage of this term.
    pub scope_note: Option<String>,
}

impl LcmptTerm {
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
    fn test_instrument_new() {
        let instr = Instrument::new("Violin");
        assert_eq!(instr.name, "Violin");
        assert!(instr.abbreviation.is_none());
    }

    #[test]
    fn test_lcmpt_term_new() {
        let term = LcmptTerm::new(
            "http://id.loc.gov/authorities/performanceMediums/mp2013015550",
            "violin",
        );
        assert_eq!(term.label, "violin");
        assert!(term.broader_uri.is_none());
    }

    #[test]
    fn test_lcmpt_term_with_broader() {
        let term = LcmptTerm::new(
            "http://id.loc.gov/authorities/performanceMediums/mp2013015550",
            "violin",
        )
        .with_broader("http://id.loc.gov/authorities/performanceMediums/mp2013015518")
        .with_scope_note("A bowed string instrument");
        assert_eq!(
            term.broader_uri,
            Some("http://id.loc.gov/authorities/performanceMediums/mp2013015518".to_string())
        );
        assert!(term.scope_note.is_some());
    }
}
