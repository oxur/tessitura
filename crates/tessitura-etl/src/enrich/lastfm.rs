//! Last.fm enrichment stage.
//!
//! Fetches folksonomy tags from the Last.fm API for artists and tracks.
//! Tags are community-driven genre/mood/style labels with associated
//! popularity counts. The enricher normalises counts to confidence
//! scores (0.0--1.0) and filters out low-count noise. All findings are
//! stored as provenance-tracked [`Assertion`]s with [`Source::LastFm`].
//!
//! [`Assertion`]: tessitura_core::provenance::Assertion
//! [`Source::LastFm`]: tessitura_core::provenance::Source::LastFm

use std::path::Path;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use tessitura_core::provenance::{Assertion, Source};
use tessitura_core::schema::Database;

use crate::enrich::resilience::RateLimiter;
use crate::error::{EnrichError, EnrichResult};

const LASTFM_API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";

/// Minimum tag count to include (filters noise from low-vote tags).
const MIN_TAG_COUNT: u32 = 10;

// ---------------------------------------------------------------------------
// API response types (private -- Last.fm nests JSON awkwardly)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TopTagsResponse {
    toptags: TopTags,
}

#[derive(Debug, Deserialize)]
struct TopTags {
    #[serde(default)]
    tag: Vec<LastFmTag>,
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single folksonomy tag returned by the Last.fm API.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmTag {
    /// Human-readable tag name (e.g. "classical", "piano").
    pub name: String,
    /// Number of users who applied this tag.
    pub count: u32,
}

/// Last.fm API client.
///
/// Wraps an HTTP client, an API key, and a rate limiter. The Last.fm API
/// allows up to 5 requests per second for non-commercial use.
#[derive(Debug, Clone)]
pub struct LastFmClient {
    http: Client,
    api_key: String,
    rate_limiter: RateLimiter,
}

impl LastFmClient {
    /// Create a new Last.fm API client.
    ///
    /// The `api_key` must be a valid Last.fm API key obtained from
    /// <https://www.last.fm/api/account/create>.
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::builder()
                .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            api_key,
            rate_limiter: RateLimiter::new(5),
        }
    }

    /// Get top tags for a track.
    ///
    /// Calls the `track.getTopTags` Last.fm API method and returns the
    /// raw tag list (unfiltered).
    pub async fn get_track_tags(&self, artist: &str, track: &str) -> EnrichResult<Vec<LastFmTag>> {
        self.rate_limiter.acquire().await;

        let response = self
            .http
            .get(LASTFM_API_BASE)
            .query(&[
                ("method", "track.getTopTags"),
                ("artist", artist),
                ("track", track),
                ("api_key", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Last.fm".to_string(),
                message: e.to_string(),
            })?;

        let result: TopTagsResponse = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Last.fm".to_string(),
            message: e.to_string(),
        })?;

        Ok(result.toptags.tag)
    }

    /// Get top tags for an artist.
    ///
    /// Calls the `artist.getTopTags` Last.fm API method and returns the
    /// raw tag list (unfiltered).
    pub async fn get_artist_tags(&self, artist: &str) -> EnrichResult<Vec<LastFmTag>> {
        self.rate_limiter.acquire().await;

        let response = self
            .http
            .get(LASTFM_API_BASE)
            .query(&[
                ("method", "artist.getTopTags"),
                ("artist", artist),
                ("api_key", &self.api_key),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Last.fm".to_string(),
                message: e.to_string(),
            })?;

        let result: TopTagsResponse = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Last.fm".to_string(),
            message: e.to_string(),
        })?;

        Ok(result.toptags.tag)
    }
}

/// Enriches entities with folksonomy tags from Last.fm.
///
/// For a given (artist, track) pair the enricher fetches both track-level
/// and artist-level tags, filters out low-count noise, normalises the
/// counts into 0.0--1.0 confidence scores, and persists the results as
/// [`Assertion`]s.
#[derive(Debug, Clone)]
pub struct LastFmEnricher {
    client: LastFmClient,
    min_tag_count: u32,
}

impl LastFmEnricher {
    /// Create a new Last.fm enricher.
    ///
    /// The `api_key` must be a valid Last.fm API key.
    pub fn new(api_key: String) -> Self {
        Self {
            client: LastFmClient::new(api_key),
            min_tag_count: MIN_TAG_COUNT,
        }
    }

    /// Enrich an entity with folksonomy tags for a track.
    ///
    /// Fetches top tags for both the track and the artist, filters by
    /// minimum count, normalises to confidence scores, and persists
    /// all results as assertions in the database.
    ///
    /// Returns the list of assertions that were created.
    pub async fn enrich(
        &self,
        artist: &str,
        track: &str,
        entity_id: &str,
        db_path: &Path,
    ) -> EnrichResult<Vec<Assertion>> {
        let mut assertions = Vec::new();

        // Track tags
        match self.client.get_track_tags(artist, track).await {
            Ok(tags) => {
                let filtered = self.tags_to_assertions(&tags, entity_id, "track");
                assertions.extend(filtered);
            }
            Err(e) => {
                log::warn!(
                    "Failed to get Last.fm track tags for {} - {}: {}",
                    artist,
                    track,
                    e
                );
            }
        }

        // Artist tags
        match self.client.get_artist_tags(artist).await {
            Ok(tags) => {
                let filtered = self.tags_to_assertions(&tags, entity_id, "artist");
                assertions.extend(filtered);
            }
            Err(e) => {
                log::warn!("Failed to get Last.fm artist tags for {}: {}", artist, e);
            }
        }

        // Persist all assertions to the database
        let db = Database::open(db_path)?;
        for assertion in &assertions {
            db.insert_assertion(assertion)?;
        }

        Ok(assertions)
    }

    /// Convert a list of tags into assertions, filtering by minimum count
    /// and normalising counts to confidence scores.
    fn tags_to_assertions(
        &self,
        tags: &[LastFmTag],
        entity_id: &str,
        scope: &str,
    ) -> Vec<Assertion> {
        let max_count = tags.iter().map(|t| t.count).max().unwrap_or(1);
        tags.iter()
            .filter(|tag| tag.count >= self.min_tag_count)
            .map(|tag| {
                let confidence = f64::from(tag.count) / f64::from(max_count);
                Assertion::new(
                    entity_id,
                    "tag",
                    serde_json::json!({
                        "name": tag.name,
                        "count": tag.count,
                        "scope": scope,
                    }),
                    Source::LastFm,
                )
                .with_confidence(confidence)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lastfm_client_creation() {
        let client = LastFmClient::new("test-key".to_string());
        let debug = format!("{:?}", client);
        assert!(debug.contains("LastFmClient"));
        assert!(debug.contains("RateLimiter"));
    }

    #[test]
    fn test_lastfm_enricher_creation() {
        let enricher = LastFmEnricher::new("test-key".to_string());
        let debug = format!("{:?}", enricher);
        assert!(debug.contains("LastFmEnricher"));
        assert!(debug.contains("LastFmClient"));
    }

    #[test]
    fn test_top_tags_deserialize() {
        let json = r#"{
            "toptags": {
                "tag": [
                    {"name": "classical", "count": 100},
                    {"name": "piano", "count": 50},
                    {"name": "romantic", "count": 5}
                ]
            }
        }"#;
        let result: TopTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(result.toptags.tag.len(), 3);
        assert_eq!(result.toptags.tag[0].name, "classical");
        assert_eq!(result.toptags.tag[0].count, 100);
    }

    #[test]
    fn test_top_tags_empty() {
        let json = r#"{"toptags": {"tag": []}}"#;
        let result: TopTagsResponse = serde_json::from_str(json).unwrap();
        assert!(result.toptags.tag.is_empty());
    }

    #[test]
    fn test_top_tags_missing_tag_field_defaults_to_empty() {
        let json = r#"{"toptags": {}}"#;
        let result: TopTagsResponse = serde_json::from_str(json).unwrap();
        assert!(result.toptags.tag.is_empty());
    }

    #[test]
    fn test_tags_to_assertions_filters_low_count() {
        let enricher = LastFmEnricher::new("key".to_string());
        let tags = vec![
            LastFmTag {
                name: "classical".to_string(),
                count: 100,
            },
            LastFmTag {
                name: "piano".to_string(),
                count: 50,
            },
            LastFmTag {
                name: "noise".to_string(),
                count: 3,
            },
        ];

        let assertions = enricher.tags_to_assertions(&tags, "entity-1", "track");

        // Only "classical" (100) and "piano" (50) pass the MIN_TAG_COUNT=10 filter
        assert_eq!(assertions.len(), 2);
        assert_eq!(assertions[0].field, "tag");
        assert_eq!(assertions[0].source, Source::LastFm);
        assert_eq!(assertions[0].value["name"], "classical");
        assert_eq!(assertions[0].value["scope"], "track");
        assert_eq!(assertions[1].value["name"], "piano");
    }

    #[test]
    fn test_tags_to_assertions_normalises_confidence() {
        let enricher = LastFmEnricher::new("key".to_string());
        let tags = vec![
            LastFmTag {
                name: "rock".to_string(),
                count: 200,
            },
            LastFmTag {
                name: "indie".to_string(),
                count: 100,
            },
        ];

        let assertions = enricher.tags_to_assertions(&tags, "e-1", "artist");

        // rock: 200/200 = 1.0, indie: 100/200 = 0.5
        assert_eq!(assertions[0].confidence, Some(1.0));
        assert_eq!(assertions[1].confidence, Some(0.5));
    }

    #[test]
    fn test_tags_to_assertions_empty_input() {
        let enricher = LastFmEnricher::new("key".to_string());
        let assertions = enricher.tags_to_assertions(&[], "e-1", "track");
        assert!(assertions.is_empty());
    }

    #[test]
    fn test_tags_to_assertions_all_below_threshold() {
        let enricher = LastFmEnricher::new("key".to_string());
        let tags = vec![
            LastFmTag {
                name: "obscure".to_string(),
                count: 2,
            },
            LastFmTag {
                name: "niche".to_string(),
                count: 5,
            },
        ];

        let assertions = enricher.tags_to_assertions(&tags, "e-1", "track");
        assert!(assertions.is_empty());
    }

    #[test]
    fn test_lastfm_tag_clone() {
        let tag = LastFmTag {
            name: "jazz".to_string(),
            count: 42,
        };
        let cloned = tag.clone();
        assert_eq!(cloned.name, "jazz");
        assert_eq!(cloned.count, 42);
    }
}
