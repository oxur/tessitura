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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_new() {
        let instr = Instrument::new("Violin");
        assert_eq!(instr.name, "Violin");
        assert!(instr.abbreviation.is_none());
    }
}
