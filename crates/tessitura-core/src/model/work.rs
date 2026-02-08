use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::ids::WorkId;

/// A distinct musical work (composition).
///
/// Corresponds to the FRBR "Work" level. A Work is an abstract
/// intellectual creation, independent of any specific performance
/// or recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work {
    pub id: WorkId,
    pub title: String,
    pub composer: Option<String>,

    /// MusicBrainz work ID, if identified.
    pub musicbrainz_id: Option<String>,

    /// Catalog number (BWV, K., Sz., BB., Op., etc.).
    pub catalog_number: Option<String>,

    /// Musical key (e.g., "A minor", "D major").
    pub key: Option<String>,

    /// Year or approximate date of composition.
    pub composed_year: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Work {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: WorkId::new(),
            title: title.into(),
            composer: None,
            musicbrainz_id: None,
            catalog_number: None,
            key: None,
            composed_year: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_composer(mut self, composer: impl Into<String>) -> Self {
        self.composer = Some(composer.into());
        self
    }

    #[must_use]
    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    #[must_use]
    pub fn with_catalog_number(mut self, catalog: impl Into<String>) -> Self {
        self.catalog_number = Some(catalog.into());
        self
    }

    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    #[must_use]
    pub fn with_composed_year(mut self, year: i32) -> Self {
        self.composed_year = Some(year);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_new() {
        let work = Work::new("Symphony No. 5");
        assert_eq!(work.title, "Symphony No. 5");
        assert!(work.composer.is_none());
    }

    #[test]
    fn test_work_builder() {
        let work = Work::new("String Quartet No. 4")
            .with_composer("Béla Bartók")
            .with_catalog_number("Sz.91")
            .with_key("C major")
            .with_composed_year(1928);

        assert_eq!(work.composer, Some("Béla Bartók".to_string()));
        assert_eq!(work.catalog_number, Some("Sz.91".to_string()));
        assert_eq!(work.key, Some("C major".to_string()));
        assert_eq!(work.composed_year, Some(1928));
    }
}
