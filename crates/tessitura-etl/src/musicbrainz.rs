use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Response types: Recording
// ---------------------------------------------------------------------------

/// A recording as returned by the MusicBrainz API.
#[derive(Debug, Deserialize)]
pub struct MbRecording {
    pub id: String,
    pub title: String,
    #[serde(rename = "artist-credit")]
    pub artist_credit: Option<Vec<MbArtistCredit>>,
    pub releases: Option<Vec<MbRelease>>,
    #[serde(default)]
    pub relations: Vec<MbRelation>,
}

/// An artist credit entry on a recording.
#[derive(Debug, Deserialize)]
pub struct MbArtistCredit {
    pub artist: MbArtist,
}

/// An artist as returned by MusicBrainz.
#[derive(Debug, Deserialize)]
pub struct MbArtist {
    pub id: String,
    pub name: String,
}

/// A release (album) summary returned with a recording.
#[derive(Debug, Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    #[serde(rename = "release-group")]
    pub release_group: Option<MbReleaseGroup>,
}

/// A release group ID returned with a release.
#[derive(Debug, Deserialize)]
pub struct MbReleaseGroup {
    pub id: String,
}

/// A relation on a recording (e.g. "performance" linking to a work).
#[derive(Debug, Deserialize)]
pub struct MbRelation {
    #[serde(rename = "type")]
    pub relation_type: String,
    pub work: Option<MbWork>,
}

/// A work summary returned in a recording relation.
#[derive(Debug, Deserialize)]
pub struct MbWork {
    pub id: String,
    pub title: String,
}

// ---------------------------------------------------------------------------
// Response types: Work detail
// ---------------------------------------------------------------------------

/// Full work details as returned by the MusicBrainz work endpoint.
#[derive(Debug, Deserialize)]
pub struct MbWorkDetail {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub relations: Vec<MbWorkRelation>,
    #[serde(default)]
    pub attributes: Vec<String>,
}

/// A relation on a work (e.g. "composer" linking to an artist).
#[derive(Debug, Deserialize)]
pub struct MbWorkRelation {
    #[serde(rename = "type")]
    pub relation_type: String,
    pub artist: Option<MbArtist>,
    #[serde(default)]
    pub attributes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Response types: Release detail
// ---------------------------------------------------------------------------

/// Full release details as returned by the MusicBrainz release endpoint.
#[derive(Debug, Deserialize)]
pub struct MbReleaseDetail {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(rename = "label-info", default)]
    pub label_info: Vec<MbLabelInfo>,
    #[serde(default)]
    pub media: Vec<MbMedia>,
}

/// Label information attached to a release.
#[derive(Debug, Deserialize)]
pub struct MbLabelInfo {
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
    pub label: Option<MbLabel>,
}

/// A record label.
#[derive(Debug, Deserialize)]
pub struct MbLabel {
    pub id: String,
    pub name: String,
}

/// A media entry (disc) on a release.
#[derive(Debug, Deserialize)]
pub struct MbMedia {
    pub position: Option<u32>,
    pub format: Option<String>,
    #[serde(rename = "track-count")]
    pub track_count: Option<u32>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// MusicBrainz API client.
///
/// Wraps a `reqwest::Client` pre-configured with the MusicBrainz user-agent
/// header and a 30-second timeout. Rate limiting is **not** enforced inside
/// the client itself; callers are expected to use a [`RateLimiter`] before
/// each request.
///
/// [`RateLimiter`]: crate::enrich::resilience::RateLimiter
#[derive(Debug, Clone)]
pub struct MusicBrainzClient {
    http: Client,
}

impl MusicBrainzClient {
    /// Create a new MusicBrainz client.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
            .build()?;

        Ok(Self { http })
    }

    /// Get recording details by MusicBrainz ID.
    ///
    /// Includes artist credits, releases, and work relations.
    ///
    /// Rate limit: 1 request/second (enforced by caller).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn get_recording(&self, mbid: &str) -> Result<MbRecording, reqwest::Error> {
        let url = format!(
            "https://musicbrainz.org/ws/2/recording/{}?inc=releases+artists+work-rels&fmt=json",
            mbid
        );

        let response = self.http.get(&url).send().await?.error_for_status()?;
        response.json::<MbRecording>().await
    }

    /// Get work details by MusicBrainz ID.
    ///
    /// Includes artist relations (composer, lyricist, etc.) and work
    /// attributes (key, etc.).
    ///
    /// Rate limit: 1 request/second (enforced by caller).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn get_work(&self, mbid: &str) -> Result<MbWorkDetail, reqwest::Error> {
        let url = format!(
            "https://musicbrainz.org/ws/2/work/{}?inc=artist-rels&fmt=json",
            mbid
        );

        let response = self.http.get(&url).send().await?.error_for_status()?;
        response.json::<MbWorkDetail>().await
    }

    /// Get release details by MusicBrainz ID.
    ///
    /// Includes label info, catalog numbers, and media (disc) information.
    ///
    /// Rate limit: 1 request/second (enforced by caller).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn get_release(&self, mbid: &str) -> Result<MbReleaseDetail, reqwest::Error> {
        let url = format!(
            "https://musicbrainz.org/ws/2/release/{}?inc=labels+media&fmt=json",
            mbid
        );

        let response = self.http.get(&url).send().await?.error_for_status()?;
        response.json::<MbReleaseDetail>().await
    }

    /// Search recordings by metadata (fallback when no fingerprint).
    ///
    /// Searches the MusicBrainz recording index by artist, title, and
    /// optionally album name. Returns up to 5 candidates.
    ///
    /// Rate limit: 1 request/second (enforced by caller).
    ///
    /// # Errors
    /// Returns an error if the API request fails or the response cannot be parsed.
    pub async fn search_recording(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Result<Vec<MbRecording>, reqwest::Error> {
        use std::fmt::Write;

        #[derive(Deserialize)]
        struct SearchResult {
            recordings: Vec<MbRecording>,
        }

        let mut query = format!("recording:\"{title}\" AND artist:\"{artist}\"");
        if let Some(album) = album {
            let _ = write!(query, " AND release:\"{album}\"");
        }

        let url = "https://musicbrainz.org/ws/2/recording/";

        let response = self
            .http
            .get(url)
            .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "5")])
            .send()
            .await?
            .error_for_status()?;

        let result = response.json::<SearchResult>().await?;
        Ok(result.recordings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musicbrainz_client_creation() {
        let client = MusicBrainzClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_mb_recording_deserialize_minimal() {
        let json = r#"{
            "id": "abc-123",
            "title": "Test Recording",
            "relations": []
        }"#;

        let recording: MbRecording = serde_json::from_str(json).unwrap();
        assert_eq!(recording.id, "abc-123");
        assert_eq!(recording.title, "Test Recording");
        assert!(recording.artist_credit.is_none());
        assert!(recording.releases.is_none());
        assert!(recording.relations.is_empty());
    }

    #[test]
    fn test_mb_recording_deserialize_full() {
        let json = r#"{
            "id": "abc-123",
            "title": "Test Recording",
            "artist-credit": [
                {
                    "artist": {
                        "id": "artist-1",
                        "name": "Test Artist"
                    }
                }
            ],
            "releases": [
                {
                    "id": "release-1",
                    "title": "Test Album",
                    "release-group": {
                        "id": "rg-1"
                    }
                }
            ],
            "relations": [
                {
                    "type": "performance",
                    "work": {
                        "id": "work-1",
                        "title": "Test Work"
                    }
                }
            ]
        }"#;

        let recording: MbRecording = serde_json::from_str(json).unwrap();
        assert_eq!(recording.artist_credit.as_ref().unwrap().len(), 1);
        assert_eq!(
            recording.artist_credit.as_ref().unwrap()[0].artist.name,
            "Test Artist"
        );
        assert_eq!(recording.releases.as_ref().unwrap().len(), 1);
        assert_eq!(recording.relations.len(), 1);
        assert_eq!(recording.relations[0].relation_type, "performance");
        assert_eq!(
            recording.relations[0].work.as_ref().unwrap().title,
            "Test Work"
        );
    }

    #[test]
    fn test_mb_work_detail_deserialize() {
        let json = r#"{
            "id": "work-1",
            "title": "Symphony No. 5",
            "attributes": ["C minor"],
            "relations": [
                {
                    "type": "composer",
                    "artist": {
                        "id": "artist-1",
                        "name": "Ludwig van Beethoven"
                    },
                    "attributes": []
                }
            ]
        }"#;

        let work: MbWorkDetail = serde_json::from_str(json).unwrap();
        assert_eq!(work.id, "work-1");
        assert_eq!(work.title, "Symphony No. 5");
        assert_eq!(work.attributes, vec!["C minor"]);
        assert_eq!(work.relations.len(), 1);
        assert_eq!(work.relations[0].relation_type, "composer");
        assert_eq!(
            work.relations[0].artist.as_ref().unwrap().name,
            "Ludwig van Beethoven"
        );
    }

    #[test]
    fn test_mb_work_detail_deserialize_minimal() {
        let json = r#"{
            "id": "work-1",
            "title": "Test Work"
        }"#;

        let work: MbWorkDetail = serde_json::from_str(json).unwrap();
        assert_eq!(work.id, "work-1");
        assert!(work.relations.is_empty());
        assert!(work.attributes.is_empty());
    }

    #[test]
    fn test_mb_release_detail_deserialize() {
        let json = r#"{
            "id": "release-1",
            "title": "Complete Symphonies",
            "date": "1998-03-15",
            "label-info": [
                {
                    "catalog-number": "453 701-2",
                    "label": {
                        "id": "label-1",
                        "name": "Deutsche Grammophon"
                    }
                }
            ],
            "media": [
                {
                    "position": 1,
                    "format": "CD",
                    "track-count": 12
                }
            ]
        }"#;

        let release: MbReleaseDetail = serde_json::from_str(json).unwrap();
        assert_eq!(release.id, "release-1");
        assert_eq!(release.title, "Complete Symphonies");
        assert_eq!(release.date, Some("1998-03-15".to_string()));
        assert_eq!(release.label_info.len(), 1);
        assert_eq!(
            release.label_info[0].catalog_number,
            Some("453 701-2".to_string())
        );
        assert_eq!(
            release.label_info[0].label.as_ref().unwrap().name,
            "Deutsche Grammophon"
        );
        assert_eq!(release.media.len(), 1);
        assert_eq!(release.media[0].position, Some(1));
        assert_eq!(release.media[0].track_count, Some(12));
    }

    #[test]
    fn test_mb_release_detail_deserialize_minimal() {
        let json = r#"{
            "id": "release-1",
            "title": "Test Release"
        }"#;

        let release: MbReleaseDetail = serde_json::from_str(json).unwrap();
        assert_eq!(release.id, "release-1");
        assert!(release.date.is_none());
        assert!(release.label_info.is_empty());
        assert!(release.media.is_empty());
    }
}
