use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::ids::ArtistId;

/// The role an artist plays in a musical context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistRole {
    Composer,
    Performer,
    Conductor,
    Ensemble,
    Producer,
    Other,
}

/// A musical artist (person or group).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub sort_name: Option<String>,

    /// `MusicBrainz` artist ID.
    pub musicbrainz_id: Option<String>,

    /// Primary role(s) this artist is known for.
    pub roles: Vec<ArtistRole>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artist {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ArtistId::new(),
            name: name.into(),
            sort_name: None,
            musicbrainz_id: None,
            roles: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_role(mut self, role: ArtistRole) -> Self {
        self.roles.push(role);
        self
    }

    #[must_use]
    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artist_new() {
        let artist = Artist::new("Miles Davis");
        assert_eq!(artist.name, "Miles Davis");
        assert!(artist.roles.is_empty());
    }

    #[test]
    fn test_artist_builder() {
        let artist = Artist::new("Herbert von Karajan")
            .with_role(ArtistRole::Conductor)
            .with_musicbrainz_id("test-mbid");

        assert_eq!(artist.roles, vec![ArtistRole::Conductor]);
        assert_eq!(artist.musicbrainz_id, Some("test-mbid".to_string()));
    }
}
