use serde::{Deserialize, Serialize};

/// A musical form (sonata, fugue, rondo, string quartet, etc.).
///
/// Distinct from genre — form describes structure, genre describes style.
/// In Phase 2, these will be mapped to LCGFT form terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Form {
    pub name: String,
    pub lcgft_uri: Option<String>,
}

impl Form {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            lcgft_uri: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_new() {
        let form = Form::new("Sonata");
        assert_eq!(form.name, "Sonata");
        assert!(form.lcgft_uri.is_none());
    }
}
