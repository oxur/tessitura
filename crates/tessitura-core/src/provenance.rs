use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The source of a metadata assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    /// Extracted from embedded audio file tags.
    EmbeddedTag,
    /// `AcoustID` fingerprint matching.
    AcoustId,
    /// `MusicBrainz` database.
    MusicBrainz,
    /// Wikidata.
    Wikidata,
    /// Last.fm folksonomy tags.
    LastFm,
    /// Library of Congress Genre/Form Terms.
    Lcgft,
    /// Library of Congress Medium of Performance Thesaurus.
    Lcmpt,
    /// Discogs database.
    Discogs,
    /// Manual entry by the user.
    User,
}

/// A metadata assertion with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assertion {
    /// The entity this assertion is about (work, expression, etc.).
    pub entity_id: String,

    /// The field being asserted (e.g., "genre", "key", "form").
    pub field: String,

    /// The asserted value.
    pub value: serde_json::Value,

    /// Where this assertion came from.
    pub source: Source,

    /// Confidence score (0.0 to 1.0), if applicable.
    pub confidence: Option<f64>,

    /// When this assertion was fetched/created.
    pub fetched_at: DateTime<Utc>,
}

impl Assertion {
    #[must_use]
    pub fn new(
        entity_id: impl Into<String>,
        field: impl Into<String>,
        value: serde_json::Value,
        source: Source,
    ) -> Self {
        Self {
            entity_id: entity_id.into(),
            field: field.into(),
            value,
            source,
            confidence: None,
            fetched_at: Utc::now(),
        }
    }

    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_assertion_new() {
        let assertion = Assertion::new("work-id", "genre", json!("Classical"), Source::MusicBrainz);

        assert_eq!(assertion.entity_id, "work-id");
        assert_eq!(assertion.field, "genre");
        assert_eq!(assertion.source, Source::MusicBrainz);
        assert!(assertion.confidence.is_none());
    }

    #[test]
    fn test_assertion_with_confidence() {
        let assertion =
            Assertion::new("id", "field", json!("value"), Source::AcoustId).with_confidence(0.95);

        assert_eq!(assertion.confidence, Some(0.95));
    }
}
