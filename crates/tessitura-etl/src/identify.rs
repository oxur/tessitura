use std::path::PathBuf;
use tessitura_core::model::{Artist, ArtistRole, Expression, Manifestation, Work};
use tessitura_core::schema::Database;
use treadle::{Stage, StageContext, StageOutcome};

use crate::acoustid::AcoustIdClient;
use crate::enrich::resilience::RateLimiter;
use crate::musicbrainz::MusicBrainzClient;

/// The Identify stage: match audio files to MusicBrainz recordings.
#[derive(Debug)]
pub struct IdentifyStage {
    acoustid: Option<AcoustIdClient>,
    musicbrainz: MusicBrainzClient,
    db_path: PathBuf,
    mb_rate_limiter: RateLimiter,
}

impl IdentifyStage {
    /// Create a new IdentifyStage.
    ///
    /// # Errors
    /// Returns an error if the clients cannot be initialized.
    pub fn new(
        acoustid_api_key: Option<String>,
        db_path: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let acoustid = if let Some(key) = acoustid_api_key {
            Some(AcoustIdClient::new(key)?)
        } else {
            None
        };

        let musicbrainz = MusicBrainzClient::new()?;

        Ok(Self {
            acoustid,
            musicbrainz,
            db_path,
            mb_rate_limiter: RateLimiter::new(1), // 1 req/sec for MusicBrainz
        })
    }

    /// Strip common remaster/edition suffixes that MusicBrainz won't have.
    /// Keep important variations like (Remix), (Live), (Radio Edit).
    fn strip_title_suffix(title: &str) -> String {
        let patterns = [
            r"\s*\(\d{4}\s+[Rr]emaster(?:ed)?\)", // (2006 Remaster), (2024 remastered)
            r"\s*\([Rr]emaster(?:ed)?\s+\d{4}\)", // (Remastered 2006)
            r"\s*\(\d+-bit\s+Studio\s+Master\)",  // (24-bit Studio Master)
            r"\s*\(Studio\s+Master\)",            // (Studio Master)
            r"\s*\(Deluxe\s+Edition\)",           // (Deluxe Edition)
            r"\s*\(Expanded\s+Edition\)",         // (Expanded Edition)
            r"\s*\(Anniversary\s+Edition\)",      // (Anniversary Edition)
            r"\s*\(Bonus\s+Track\s+Version\)",    // (Bonus Track Version)
        ];

        let mut cleaned = title.to_string();
        for pattern in &patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                cleaned = re.replace(&cleaned, "").to_string();
            }
        }
        cleaned.trim().to_string()
    }

    /// Main identification orchestration: fingerprint matching → metadata fallback → FRBR entity creation.
    #[allow(clippy::too_many_lines)] // Main workflow orchestration
    async fn identify_items(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Open database and get unidentified items (before any async)
        let unidentified = {
            let db = Database::open(&self.db_path)?;
            db.list_unidentified_items()?
        };

        log::info!("Found {} unidentified items", unidentified.len());

        let mut identified_count = 0;

        for item in unidentified {
            log::debug!("Identifying: {}", item.file_path.display());

            let mut recording_id: Option<String> = None;

            // Step 1: Try fingerprint matching if available
            if let Some(ref acoustid) = self.acoustid {
                if let (Some(ref fingerprint), Some(duration)) =
                    (&item.fingerprint, item.duration_secs)
                {
                    log::debug!(
                        "Attempting AcoustID fingerprint match for {}",
                        item.file_path.display()
                    );

                    match acoustid.lookup(fingerprint, duration).await {
                        Ok(response) => {
                            if let Some(result) = response.results.first() {
                                if let Some(recordings) = &result.recordings {
                                    if let Some(recording) = recordings.first() {
                                        recording_id = Some(recording.id.clone());
                                        log::info!(
                                            "Fingerprint match found: {} (score: {:.2})",
                                            recording.id,
                                            result.score
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "AcoustID lookup failed for {}: {}",
                                item.file_path.display(),
                                e
                            );
                        }
                    }
                }
            }

            // Step 2: Fall back to metadata-based search if no fingerprint match
            if recording_id.is_none() {
                if let (Some(ref artist), Some(ref title)) = (&item.tag_artist, &item.tag_title) {
                    log::debug!(
                        "Attempting metadata-based search for {}",
                        item.file_path.display()
                    );

                    // Try multiple search strategies
                    let search_strategies = [
                        (title.clone(), item.tag_album.clone()), // Exact title + album
                        (Self::strip_title_suffix(title), item.tag_album.clone()), // Cleaned title + album
                        (Self::strip_title_suffix(title), None), // Cleaned title, no album
                    ];

                    for (search_title, search_album) in &search_strategies {
                        self.mb_rate_limiter.acquire().await;

                        match self
                            .musicbrainz
                            .search_recording(artist, search_title, search_album.as_deref())
                            .await
                        {
                            Ok(recordings) => {
                                if let Some(recording) = recordings.first() {
                                    recording_id = Some(recording.id.clone());
                                    log::info!(
                                        "Metadata match found: {} - {}",
                                        recording.title,
                                        recording.id
                                    );
                                    break;
                                }
                            }
                            Err(e) => {
                                log::debug!(
                                    "MusicBrainz search failed for {} (strategy: {:?}): {}",
                                    item.file_path.display(),
                                    (search_title, search_album),
                                    e
                                );
                            }
                        }
                    }

                    if recording_id.is_none() {
                        log::debug!("No match found for {}", item.file_path.display());
                    }
                } else {
                    log::debug!(
                        "Insufficient metadata for search: {}",
                        item.file_path.display()
                    );
                }
            }

            // Step 3: If we have a recording ID, create FRBR entities
            if let Some(ref mb_recording_id) = recording_id {
                match self.create_frbr_entities(&item, mb_recording_id).await {
                    Ok(()) => {
                        identified_count += 1;
                        log::info!("Successfully identified: {}", item.file_path.display());
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to create FRBR entities for {}: {}",
                            item.file_path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(identified_count)
    }

    /// Create FRBR entities (Work, Expression, Manifestation, Artist) from a MusicBrainz recording.
    #[allow(clippy::too_many_lines)] // Complete FRBR entity creation workflow
    async fn create_frbr_entities(
        &self,
        item: &tessitura_core::model::Item,
        recording_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Fetch recording details from MusicBrainz
        self.mb_rate_limiter.acquire().await;
        let recording = self.musicbrainz.get_recording(recording_id).await?;

        // Open database in a scoped block for thread safety
        let db = Database::open(&self.db_path)?;

        // Step 1: Create/find artists
        let mut artist_ids = Vec::new();
        if let Some(artist_credit) = &recording.artist_credit {
            for credit in artist_credit {
                // Check if artist already exists by MusicBrainz ID (FK fix!)
                let artist_id =
                    if let Some(existing) = db.get_artist_by_musicbrainz_id(&credit.artist.id)? {
                        existing.id
                    } else {
                        // Create new artist
                        let artist = Artist::new(&credit.artist.name)
                            .with_musicbrainz_id(&credit.artist.id)
                            .with_role(ArtistRole::Performer);
                        db.insert_artist(&artist)?;
                        artist.id
                    };
                artist_ids.push(artist_id);
            }
        }

        // Step 2: Create/find work (if available from relations)
        let work_id = if let Some(work_relation) = recording
            .relations
            .iter()
            .find(|r| r.relation_type == "performance")
        {
            if let Some(ref work_data) = work_relation.work {
                // Check if work already exists
                let existing_work = db.get_work_by_musicbrainz_id(&work_data.id)?;

                if let Some(work) = existing_work {
                    work.id
                } else {
                    // Create new work
                    let work = Work::new(&work_data.title).with_musicbrainz_id(&work_data.id);
                    db.insert_work(&work)?;

                    // Fetch detailed work info for composer, etc.
                    self.mb_rate_limiter.acquire().await;
                    if let Ok(work_detail) = self.musicbrainz.get_work(&work_data.id).await {
                        let mut updated_work = work;

                        // Extract composer from relations
                        if let Some(composer_rel) = work_detail
                            .relations
                            .iter()
                            .find(|r| r.relation_type == "composer")
                        {
                            if let Some(ref artist) = composer_rel.artist {
                                updated_work.composer = Some(artist.name.clone());

                                // Create/update composer artist (FK fix!)
                                if db.get_artist_by_musicbrainz_id(&artist.id)?.is_none() {
                                    let composer_artist = Artist::new(&artist.name)
                                        .with_musicbrainz_id(&artist.id)
                                        .with_role(ArtistRole::Composer);
                                    db.insert_artist(&composer_artist)?;
                                }
                            }
                        }

                        // Extract key from attributes
                        if let Some(key) = work_detail.attributes.first() {
                            updated_work.key = Some(key.clone());
                        }

                        db.upsert_work(&updated_work)?;
                        updated_work.id
                    } else {
                        work.id
                    }
                }
            } else {
                // No work relation, create a work from recording title
                let work = Work::new(&recording.title);
                db.insert_work(&work)?;
                work.id
            }
        } else {
            // No work relation, create a work from recording title
            let work = Work::new(&recording.title);
            db.insert_work(&work)?;
            work.id
        };

        // Step 3: Create expression (recording)
        let existing_expression = db.get_expression_by_musicbrainz_id(recording_id)?;
        let expression_id = if let Some(expr) = existing_expression {
            expr.id
        } else {
            let mut expression = Expression::new(work_id)
                .with_title(&recording.title)
                .with_musicbrainz_id(recording_id);

            if let Some(duration) = item.duration_secs {
                expression = expression.with_duration(duration);
            }

            // Add performers
            for artist_id in &artist_ids {
                expression = expression.with_performer(*artist_id);
            }

            db.insert_expression(&expression)?;
            expression.id
        };

        // Step 4: Create manifestation (release) if available
        let manifestation_id = if let Some(releases) = &recording.releases {
            if let Some(release) = releases.first() {
                let existing_manifestation = db.get_manifestation_by_musicbrainz_id(&release.id)?;

                if let Some(man) = existing_manifestation {
                    Some(man.id)
                } else {
                    let manifestation =
                        Manifestation::new(&release.title).with_musicbrainz_id(&release.id);
                    db.insert_manifestation(&manifestation)?;
                    Some(manifestation.id)
                }
            } else {
                None
            }
        } else {
            None
        };

        // Step 5: Link item to expression and manifestation
        let mut updated_item = item.clone();
        updated_item.expression_id = Some(expression_id);
        updated_item.manifestation_id = manifestation_id;
        db.update_item(&updated_item)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Stage for IdentifyStage {
    fn name(&self) -> &str {
        "identify"
    }

    async fn execute(
        &self,
        _item: &dyn treadle::WorkItem,
        _context: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        log::info!("Starting identification");

        match self.identify_items().await {
            Ok(count) => {
                log::info!("Identification complete: {} items processed", count);
                Ok(StageOutcome::Complete)
            }
            Err(e) => Err(treadle::TreadleError::StageExecution(format!(
                "Identification failed: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_identify_stage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let stage = IdentifyStage::new(None, db_path);
        assert!(stage.is_ok());
    }

    #[tokio::test]
    async fn test_identify_stage_with_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let stage = IdentifyStage::new(Some("test-key".to_string()), db_path);
        assert!(stage.is_ok());
    }

    #[tokio::test]
    async fn test_identify_empty_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize database
        let _db = Database::open(&db_path).unwrap();

        let stage = IdentifyStage::new(None, db_path.clone()).unwrap();
        let result = stage.identify_items().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_strip_title_suffix() {
        assert_eq!(
            IdentifyStage::strip_title_suffix("Dixie Chicken (2006 Remaster)"),
            "Dixie Chicken"
        );
        assert_eq!(
            IdentifyStage::strip_title_suffix("Snowball (24-bit Studio Master)"),
            "Snowball"
        );
        assert_eq!(
            IdentifyStage::strip_title_suffix("Blue Monday (Remix)"),
            "Blue Monday (Remix)" // Keep remix!
        );
        assert_eq!(
            IdentifyStage::strip_title_suffix("Alive (Live)"),
            "Alive (Live)" // Keep live!
        );
    }
}
