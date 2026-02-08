use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::ids::ManifestationId;

/// A specific release (CD, LP, digital) of one or more recordings.
///
/// Corresponds to the FRBR "Manifestation" level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifestation {
    pub id: ManifestationId,
    pub title: String,

    /// `MusicBrainz` release ID.
    pub musicbrainz_id: Option<String>,

    /// Record label.
    pub label: Option<String>,

    /// Label catalog number (e.g., "455 297-2").
    pub catalog_number: Option<String>,

    /// Release year.
    pub release_year: Option<i32>,

    /// Number of tracks/discs.
    pub track_count: Option<u32>,
    pub disc_count: Option<u32>,

    /// Media format: "CD", "LP", "Digital", etc.
    pub format: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Manifestation {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ManifestationId::new(),
            title: title.into(),
            musicbrainz_id: None,
            label: None,
            catalog_number: None,
            release_year: None,
            track_count: None,
            disc_count: None,
            format: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_release_year(mut self, year: i32) -> Self {
        self.release_year = Some(year);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifestation_new() {
        let man = Manifestation::new("Kind of Blue");
        assert_eq!(man.title, "Kind of Blue");
        assert!(man.label.is_none());
    }

    #[test]
    fn test_manifestation_builder() {
        let man = Manifestation::new("String Quartets 1-6")
            .with_label("Decca")
            .with_release_year(1998);

        assert_eq!(man.label, Some("Decca".to_string()));
        assert_eq!(man.release_year, Some(1998));
    }
}
