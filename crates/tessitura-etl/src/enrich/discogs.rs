//! Discogs enrichment stage.
//!
//! Fetches release-level metadata from the Discogs API for items that have
//! a known catalog number or Discogs release ID. The enricher gathers label,
//! catalog number, release year, genre, style, format, and personnel credit
//! information. All findings are stored as provenance-tracked [`Assertion`]s.
//!
//! Rate limits are enforced internally: authenticated requests are capped at
//! 4 req/sec (240/min) and unauthenticated requests at 1 req/sec (60/min),
//! matching the Discogs API terms.
//!
//! [`Assertion`]: tessitura_core::provenance::Assertion

use std::path::Path;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use tessitura_core::provenance::{Assertion, Source};
use tessitura_core::schema::Database;

use crate::enrich::resilience::RateLimiter;
use crate::error::{EnrichError, EnrichResult};

const DISCOGS_API_BASE: &str = "https://api.discogs.com";

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Top-level search response wrapper (private).
#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<DiscogsSearchResult>,
}

/// A single search result from the Discogs database search endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsSearchResult {
    /// Discogs release ID.
    pub id: u64,
    /// Combined "Artist - Title" string.
    pub title: String,
    /// Release year, if known.
    #[serde(default)]
    pub year: Option<String>,
    /// Label names.
    #[serde(default)]
    pub label: Vec<String>,
    /// Catalog number.
    #[serde(default)]
    pub catno: Option<String>,
}

/// Full release details from the Discogs releases endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsRelease {
    /// Discogs release ID.
    pub id: u64,
    /// Release title.
    pub title: String,
    /// Release year, if known.
    #[serde(default)]
    pub year: Option<u32>,
    /// Labels associated with this release.
    #[serde(default)]
    pub labels: Vec<DiscogsLabel>,
    /// High-level genre tags (e.g. "Jazz", "Electronic").
    #[serde(default)]
    pub genres: Vec<String>,
    /// Sub-genre style tags (e.g. "Modal", "Hard Bop").
    #[serde(default)]
    pub styles: Vec<String>,
    /// Physical/digital format descriptions.
    #[serde(default)]
    pub formats: Vec<DiscogsFormat>,
    /// Credits for non-primary artists (producers, engineers, etc.).
    #[serde(default)]
    pub extraartists: Vec<DiscogsExtraArtist>,
}

/// A record label entry on a Discogs release.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsLabel {
    /// Discogs label ID, if available.
    pub id: Option<u64>,
    /// Label name.
    pub name: String,
    /// Catalog number for this label on this release.
    #[serde(default)]
    pub catno: Option<String>,
}

/// A format entry describing the physical/digital medium.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsFormat {
    /// Format name (e.g. "CD", "Vinyl", "File").
    pub name: String,
    /// Additional descriptions (e.g. "Album", "Remastered", "Stereo").
    #[serde(default)]
    pub descriptions: Vec<String>,
}

/// An extra artist credit on a Discogs release.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsExtraArtist {
    /// Artist name.
    pub name: String,
    /// Credit role (e.g. "Producer", "Mastered By").
    pub role: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Discogs API client.
///
/// Wraps a [`reqwest::Client`] pre-configured with the required user-agent
/// header and a 30-second timeout. Authentication is optional; when a
/// personal access token is provided, the higher authenticated rate limit
/// (240 req/min) is used.
///
/// Rate limiting is enforced inside the client via a [`RateLimiter`].
///
/// [`RateLimiter`]: crate::enrich::resilience::RateLimiter
#[derive(Debug, Clone)]
pub struct DiscogsClient {
    http: Client,
    token: Option<String>,
    rate_limiter: RateLimiter,
}

impl DiscogsClient {
    /// Create a new Discogs client.
    ///
    /// If a personal access token is provided, the client uses the
    /// authenticated rate limit of 4 req/sec (240/min). Without a token,
    /// the unauthenticated limit of 1 req/sec (60/min) applies.
    pub fn new(token: Option<String>) -> Self {
        let rps = if token.is_some() { 4 } else { 1 };
        Self {
            http: Client::builder()
                .user_agent("tessitura/0.1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            token,
            rate_limiter: RateLimiter::new(rps),
        }
    }

    /// Build the `Authorization` header value, if a token is configured.
    fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Discogs token={t}"))
    }

    /// Search for releases by catalog number.
    pub async fn search_release(&self, catno: &str) -> EnrichResult<Vec<DiscogsSearchResult>> {
        self.rate_limiter.acquire().await;

        let mut request = self
            .http
            .get(format!("{DISCOGS_API_BASE}/database/search"))
            .query(&[("catno", catno), ("type", "release")]);

        if let Some(auth) = self.auth_header() {
            request = request.header("Authorization", auth);
        }

        let response = request
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Discogs".to_string(),
                message: e.to_string(),
            })?;

        let result: SearchResponse = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Discogs".to_string(),
            message: e.to_string(),
        })?;

        Ok(result.results)
    }

    /// Get release details by Discogs release ID.
    pub async fn get_release(&self, id: u64) -> EnrichResult<DiscogsRelease> {
        self.rate_limiter.acquire().await;

        let mut request = self.http.get(format!("{DISCOGS_API_BASE}/releases/{id}"));

        if let Some(auth) = self.auth_header() {
            request = request.header("Authorization", auth);
        }

        let response = request
            .send()
            .await?
            .error_for_status()
            .map_err(|e| EnrichError::Http {
                source_name: "Discogs".to_string(),
                message: e.to_string(),
            })?;

        let release: DiscogsRelease = response.json().await.map_err(|e| EnrichError::Parse {
            source_name: "Discogs".to_string(),
            message: e.to_string(),
        })?;

        Ok(release)
    }
}

// ---------------------------------------------------------------------------
// Enricher
// ---------------------------------------------------------------------------

/// Enriches entities with release-level metadata from the Discogs database.
///
/// The enricher wraps a [`DiscogsClient`] and converts API responses into
/// provenance-tracked [`Assertion`]s stored in the database. Two entry
/// points are provided:
///
/// - [`enrich_by_catno`](Self::enrich_by_catno) -- search by catalog number
///   then fetch the full release.
/// - [`enrich_release`](Self::enrich_release) -- fetch a known Discogs
///   release ID directly.
#[derive(Debug, Clone)]
pub struct DiscogsEnricher {
    client: DiscogsClient,
}

impl DiscogsEnricher {
    /// Create a new Discogs enricher.
    ///
    /// The optional `token` is a Discogs personal access token used for
    /// authentication. Authenticated requests benefit from a higher rate
    /// limit (240 req/min vs 60 req/min).
    pub fn new(token: Option<String>) -> Self {
        Self {
            client: DiscogsClient::new(token),
        }
    }

    /// Enrich an entity by searching for a release by catalog number.
    ///
    /// Searches Discogs for the given catalog number, takes the first
    /// matching release, fetches its full details, and creates assertions
    /// for all available metadata.
    ///
    /// Returns an empty `Vec` if no release is found for the catalog number.
    pub async fn enrich_by_catno(
        &self,
        catno: &str,
        entity_id: &str,
        db_path: &Path,
    ) -> EnrichResult<Vec<Assertion>> {
        let results = self.client.search_release(catno).await?;

        let Some(result) = results.first() else {
            log::debug!("No Discogs release found for catalog number {}", catno);
            return Ok(Vec::new());
        };
        let release_id = result.id;

        self.enrich_release(release_id, entity_id, db_path).await
    }

    /// Enrich an entity from a specific Discogs release ID.
    ///
    /// Fetches the full release details and creates assertions for labels,
    /// catalog numbers, release year, genres, styles, formats, and
    /// personnel credits.
    pub async fn enrich_release(
        &self,
        release_id: u64,
        entity_id: &str,
        db_path: &Path,
    ) -> EnrichResult<Vec<Assertion>> {
        let release = self.client.get_release(release_id).await?;
        let mut assertions = Vec::new();

        // Labels and catalog numbers
        for label in &release.labels {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "label",
                    serde_json::json!({
                        "name": label.name,
                        "discogs_id": label.id,
                    }),
                    Source::Discogs,
                )
                .with_confidence(0.9),
            );

            if let Some(catno) = &label.catno {
                assertions.push(
                    Assertion::new(
                        entity_id,
                        "catalog_number",
                        serde_json::json!(catno),
                        Source::Discogs,
                    )
                    .with_confidence(0.9),
                );
            }
        }

        // Release year
        if let Some(year) = release.year {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "release_year",
                    serde_json::json!(year),
                    Source::Discogs,
                )
                .with_confidence(0.9),
            );
        }

        // Genres
        for genre in &release.genres {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "genre",
                    serde_json::json!(genre),
                    Source::Discogs,
                )
                .with_confidence(0.8),
            );
        }

        // Styles (sub-genres)
        for style in &release.styles {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "style",
                    serde_json::json!(style),
                    Source::Discogs,
                )
                .with_confidence(0.8),
            );
        }

        // Formats (CD, LP, etc.)
        for format in &release.formats {
            assertions.push(Assertion::new(
                entity_id,
                "format",
                serde_json::json!({
                    "name": format.name,
                    "descriptions": format.descriptions,
                }),
                Source::Discogs,
            ));
        }

        // Personnel credits (extra artists)
        for artist in &release.extraartists {
            assertions.push(
                Assertion::new(
                    entity_id,
                    "personnel",
                    serde_json::json!({
                        "name": artist.name,
                        "role": artist.role,
                    }),
                    Source::Discogs,
                )
                .with_confidence(0.85),
            );
        }

        // Persist all assertions to the database
        let db = Database::open(db_path)?;
        for assertion in &assertions {
            db.insert_assertion(assertion)?;
        }

        Ok(assertions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discogs_client_creation_unauthenticated() {
        let client = DiscogsClient::new(None);
        let debug = format!("{client:?}");
        assert!(debug.contains("DiscogsClient"));
        assert!(debug.contains("RateLimiter"));
    }

    #[test]
    fn test_discogs_client_creation_authenticated() {
        let client = DiscogsClient::new(Some("test-token".to_string()));
        assert!(client.token.is_some());
        assert_eq!(client.token.as_deref(), Some("test-token"));
    }

    #[test]
    fn test_discogs_client_auth_header_with_token() {
        let client = DiscogsClient::new(Some("my-secret-token".to_string()));
        let header = client.auth_header();
        assert_eq!(header, Some("Discogs token=my-secret-token".to_string()));
    }

    #[test]
    fn test_discogs_client_auth_header_without_token() {
        let client = DiscogsClient::new(None);
        assert!(client.auth_header().is_none());
    }

    #[test]
    fn test_discogs_enricher_creation() {
        let enricher = DiscogsEnricher::new(None);
        let debug = format!("{enricher:?}");
        assert!(debug.contains("DiscogsEnricher"));
        assert!(debug.contains("DiscogsClient"));
    }

    #[test]
    fn test_discogs_enricher_creation_with_token() {
        let enricher = DiscogsEnricher::new(Some("tok".to_string()));
        let debug = format!("{enricher:?}");
        assert!(debug.contains("DiscogsEnricher"));
    }

    #[test]
    fn test_search_response_deserialize() {
        let json = r#"{
            "results": [
                {
                    "id": 12345,
                    "title": "Kind of Blue",
                    "year": "1959",
                    "label": ["Columbia"],
                    "catno": "CS 8163"
                }
            ]
        }"#;
        let result: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].id, 12345);
        assert_eq!(result.results[0].title, "Kind of Blue");
        assert_eq!(result.results[0].year, Some("1959".to_string()));
        assert_eq!(result.results[0].label, vec!["Columbia".to_string()]);
        assert_eq!(result.results[0].catno, Some("CS 8163".to_string()));
    }

    #[test]
    fn test_search_response_deserialize_empty() {
        let json = r#"{"results": []}"#;
        let result: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(result.results.is_empty());
    }

    #[test]
    fn test_search_result_deserialize_minimal() {
        let json = r#"{"id": 1, "title": "Test"}"#;
        let result: DiscogsSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.id, 1);
        assert_eq!(result.title, "Test");
        assert!(result.year.is_none());
        assert!(result.label.is_empty());
        assert!(result.catno.is_none());
    }

    #[test]
    fn test_release_deserialize_full() {
        let json = r#"{
            "id": 12345,
            "title": "Kind of Blue",
            "year": 1959,
            "labels": [
                {"id": 1, "name": "Columbia", "catno": "CS 8163"}
            ],
            "genres": ["Jazz"],
            "styles": ["Modal"],
            "formats": [
                {"name": "CD", "descriptions": ["Album", "Remastered"]}
            ],
            "extraartists": [
                {"name": "Teo Macero", "role": "Producer"}
            ]
        }"#;
        let release: DiscogsRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.id, 12345);
        assert_eq!(release.title, "Kind of Blue");
        assert_eq!(release.year, Some(1959));
        assert_eq!(release.labels.len(), 1);
        assert_eq!(release.labels[0].name, "Columbia");
        assert_eq!(release.labels[0].id, Some(1));
        assert_eq!(release.labels[0].catno, Some("CS 8163".to_string()));
        assert_eq!(release.genres, vec!["Jazz"]);
        assert_eq!(release.styles, vec!["Modal"]);
        assert_eq!(release.formats.len(), 1);
        assert_eq!(release.formats[0].name, "CD");
        assert_eq!(release.formats[0].descriptions, vec!["Album", "Remastered"]);
        assert_eq!(release.extraartists.len(), 1);
        assert_eq!(release.extraartists[0].name, "Teo Macero");
        assert_eq!(release.extraartists[0].role, "Producer");
    }

    #[test]
    fn test_release_deserialize_minimal() {
        let json = r#"{"id": 1, "title": "Test"}"#;
        let release: DiscogsRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.id, 1);
        assert_eq!(release.title, "Test");
        assert!(release.year.is_none());
        assert!(release.labels.is_empty());
        assert!(release.genres.is_empty());
        assert!(release.styles.is_empty());
        assert!(release.formats.is_empty());
        assert!(release.extraartists.is_empty());
    }

    #[test]
    fn test_release_deserialize_multiple_labels() {
        let json = r#"{
            "id": 99,
            "title": "Multi-Label Release",
            "labels": [
                {"id": 10, "name": "Label A", "catno": "CAT-001"},
                {"id": 20, "name": "Label B"}
            ]
        }"#;
        let release: DiscogsRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.labels.len(), 2);
        assert_eq!(release.labels[0].name, "Label A");
        assert_eq!(release.labels[0].catno, Some("CAT-001".to_string()));
        assert_eq!(release.labels[1].name, "Label B");
        assert!(release.labels[1].catno.is_none());
    }

    #[test]
    fn test_release_deserialize_multiple_genres_and_styles() {
        let json = r#"{
            "id": 42,
            "title": "Genre Mix",
            "genres": ["Electronic", "Hip Hop"],
            "styles": ["Ambient", "Trip Hop", "Downtempo"]
        }"#;
        let release: DiscogsRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.genres, vec!["Electronic", "Hip Hop"]);
        assert_eq!(release.styles, vec!["Ambient", "Trip Hop", "Downtempo"]);
    }

    #[test]
    fn test_format_deserialize_no_descriptions() {
        let json = r#"{"name": "Vinyl"}"#;
        let format: DiscogsFormat = serde_json::from_str(json).unwrap();
        assert_eq!(format.name, "Vinyl");
        assert!(format.descriptions.is_empty());
    }

    #[test]
    fn test_label_deserialize_no_id() {
        let json = r#"{"name": "Unknown Label"}"#;
        let label: DiscogsLabel = serde_json::from_str(json).unwrap();
        assert!(label.id.is_none());
        assert_eq!(label.name, "Unknown Label");
        assert!(label.catno.is_none());
    }

    #[test]
    fn test_extra_artist_deserialize() {
        let json = r#"{"name": "Rudy Van Gelder", "role": "Recorded By"}"#;
        let artist: DiscogsExtraArtist = serde_json::from_str(json).unwrap();
        assert_eq!(artist.name, "Rudy Van Gelder");
        assert_eq!(artist.role, "Recorded By");
    }
}
