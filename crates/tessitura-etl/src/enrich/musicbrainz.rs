//! MusicBrainz enrichment stage.
//!
//! Fetches metadata from the MusicBrainz API for items that have been
//! identified (linked to an expression with a MusicBrainz recording ID).
//! The enricher follows relations from the recording to the associated
//! work and release, gathering composer, key, label, catalog number, and
//! release year information. All findings are stored as provenance-tracked
//! [`Assertion`]s.
//!
//! [`Assertion`]: tessitura_core::provenance::Assertion

use std::path::Path;

use tessitura_core::provenance::{Assertion, Source};
use tessitura_core::schema::Database;

use crate::enrich::resilience::RateLimiter;
use crate::error::EnrichResult;
use crate::musicbrainz::MusicBrainzClient;

/// Enriches entities with metadata fetched from the MusicBrainz API.
///
/// The enricher wraps a [`MusicBrainzClient`] and a per-source
/// [`RateLimiter`] (1 req/sec as required by MusicBrainz). A single
/// enrichment pass may issue multiple API calls (recording, work, release)
/// so each call is rate-limited independently.
#[derive(Debug, Clone)]
pub struct MusicBrainzEnricher {
    client: MusicBrainzClient,
    rate_limiter: RateLimiter,
}

impl MusicBrainzEnricher {
    /// Create a new MusicBrainz enricher.
    ///
    /// Constructs an HTTP client and a rate limiter that enforces the
    /// MusicBrainz API limit of 1 request per second.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP client cannot be created.
    pub fn new() -> EnrichResult<Self> {
        let client = MusicBrainzClient::new()?;
        Ok(Self {
            client,
            rate_limiter: RateLimiter::new(1),
        })
    }

    /// Enrich a single entity by its MusicBrainz recording ID.
    ///
    /// Fetches the recording, then follows work and release relations to
    /// gather comprehensive metadata. All findings are stored as
    /// provenance-tracked assertions in the database.
    ///
    /// Returns the list of assertions that were created.
    ///
    /// # Errors
    /// Returns an error on HTTP failure, parse failure, or database write
    /// failure.
    pub async fn enrich_recording(
        &self,
        recording_mbid: &str,
        entity_id: &str,
        db_path: &Path,
    ) -> EnrichResult<Vec<Assertion>> {
        let mut assertions = Vec::new();

        // 1. Fetch recording details
        self.rate_limiter.acquire().await;
        let recording = self.client.get_recording(recording_mbid).await?;

        // Store recording title
        assertions.push(Assertion::new(
            entity_id,
            "title",
            serde_json::json!(recording.title),
            Source::MusicBrainz,
        ));

        // Store artist assertions from artist credits
        if let Some(credits) = &recording.artist_credit {
            for credit in credits {
                assertions.push(Assertion::new(
                    entity_id,
                    "artist",
                    serde_json::json!({
                        "name": credit.artist.name,
                        "musicbrainz_id": credit.artist.id,
                    }),
                    Source::MusicBrainz,
                ));
            }
        }

        // 2. Follow work relations (recording -> work)
        for relation in &recording.relations {
            if relation.relation_type == "performance" {
                if let Some(work) = &relation.work {
                    let work_assertions = self.enrich_work(&work.id, entity_id).await?;
                    assertions.extend(work_assertions);
                }
            }
        }

        // 3. Follow release relations (enrich the first/primary release)
        if let Some(releases) = &recording.releases {
            if let Some(release) = releases.first() {
                let release_assertions = self.enrich_release(&release.id, entity_id).await?;
                assertions.extend(release_assertions);
            }
        }

        // 4. Persist all assertions to the database
        let db = Database::open(db_path)?;
        for assertion in &assertions {
            db.insert_assertion(assertion)?;
        }

        Ok(assertions)
    }

    /// Fetch work details and create assertions for composer, key, etc.
    async fn enrich_work(&self, work_mbid: &str, entity_id: &str) -> EnrichResult<Vec<Assertion>> {
        let mut assertions = Vec::new();

        self.rate_limiter.acquire().await;
        let work = self.client.get_work(work_mbid).await?;

        // Work title
        assertions.push(Assertion::new(
            entity_id,
            "work_title",
            serde_json::json!(work.title),
            Source::MusicBrainz,
        ));

        // Work MusicBrainz ID
        assertions.push(Assertion::new(
            entity_id,
            "work_musicbrainz_id",
            serde_json::json!(work.id),
            Source::MusicBrainz,
        ));

        // Extract key from attributes (e.g. "A minor", "D major")
        for attr in &work.attributes {
            let lower = attr.to_lowercase();
            if lower.contains("major") || lower.contains("minor") {
                assertions.push(
                    Assertion::new(
                        entity_id,
                        "key",
                        serde_json::json!(attr),
                        Source::MusicBrainz,
                    )
                    .with_confidence(0.9),
                );
            }
        }

        // Extract composer from relations
        for relation in &work.relations {
            if relation.relation_type == "composer" {
                if let Some(artist) = &relation.artist {
                    assertions.push(
                        Assertion::new(
                            entity_id,
                            "composer",
                            serde_json::json!({
                                "name": artist.name,
                                "musicbrainz_id": artist.id,
                            }),
                            Source::MusicBrainz,
                        )
                        .with_confidence(0.95),
                    );
                }
            }
        }

        Ok(assertions)
    }

    /// Fetch release details and create assertions for label, catalog
    /// number, release year, etc.
    async fn enrich_release(
        &self,
        release_mbid: &str,
        entity_id: &str,
    ) -> EnrichResult<Vec<Assertion>> {
        let mut assertions = Vec::new();

        self.rate_limiter.acquire().await;
        let release = self.client.get_release(release_mbid).await?;

        // Release year (extract from date string like "1998-03-15")
        if let Some(date) = &release.date {
            if let Some(year_str) = date.split('-').next() {
                if let Ok(year) = year_str.parse::<i32>() {
                    assertions.push(
                        Assertion::new(
                            entity_id,
                            "release_year",
                            serde_json::json!(year),
                            Source::MusicBrainz,
                        )
                        .with_confidence(0.95),
                    );
                }
            }
        }

        // Label and catalog number
        for label_info in &release.label_info {
            if let Some(label) = &label_info.label {
                assertions.push(
                    Assertion::new(
                        entity_id,
                        "label",
                        serde_json::json!({
                            "name": label.name,
                            "musicbrainz_id": label.id,
                        }),
                        Source::MusicBrainz,
                    )
                    .with_confidence(0.95),
                );
            }

            if let Some(catno) = &label_info.catalog_number {
                assertions.push(
                    Assertion::new(
                        entity_id,
                        "catalog_number",
                        serde_json::json!(catno),
                        Source::MusicBrainz,
                    )
                    .with_confidence(0.95),
                );
            }
        }

        Ok(assertions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enricher_creation_succeeds() {
        let enricher = MusicBrainzEnricher::new();
        assert!(enricher.is_ok());
    }

    #[test]
    fn test_enricher_has_rate_limiter() {
        let enricher = MusicBrainzEnricher::new().unwrap();
        // The enricher should be Debug-printable (rate limiter included).
        let debug = format!("{:?}", enricher);
        assert!(debug.contains("MusicBrainzEnricher"));
        assert!(debug.contains("RateLimiter"));
    }
}
