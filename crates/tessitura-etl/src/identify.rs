use std::path::PathBuf;
use tessitura_core::schema::Database;
use treadle::{Stage, StageContext, StageOutcome};

use crate::acoustid::AcoustIdClient;
use crate::musicbrainz::MusicBrainzClient;

/// The Identify stage: match audio files to MusicBrainz recordings.
#[derive(Debug)]
pub struct IdentifyStage {
    acoustid: Option<AcoustIdClient>,
    #[allow(dead_code)] // Will be used when MB integration is completed
    musicbrainz: MusicBrainzClient,
    db_path: PathBuf,
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
        })
    }

    async fn identify_items(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Open database and get unidentified items (before any async)
        let unidentified = {
            let db = Database::open(&self.db_path)?;
            db.list_unidentified_items()?
        };

        tracing::info!("Found {} unidentified items", unidentified.len());

        let mut identified_count = 0;

        for item in unidentified {
            tracing::debug!("Identifying: {}", item.file_path.display());

            // TODO: Try fingerprint matching if available
            if let Some(ref _acoustid) = self.acoustid {
                if let Some(ref fingerprint) = item.fingerprint {
                    if let Some(duration) = item.duration_secs {
                        tracing::debug!(
                            "Fingerprint available for {}, but matching not yet implemented",
                            item.file_path.display()
                        );
                        let _ = (fingerprint, duration); // Suppress unused warnings
                                                          // TODO: Implement in Phase 1.6 completion
                                                          // let response = self.acoustid.lookup(fingerprint, duration).await?;
                    }
                }
            }

            // TODO: Fall back to metadata-based matching
            // For now, we'll just log that the item needs identification
            tracing::debug!(
                "Metadata matching not yet implemented for {}",
                item.file_path.display()
            );

            // TODO: If we got a MusicBrainz recording ID:
            // 1. Fetch recording details from MusicBrainz
            // 2. Create/update Work, Expression, Manifestation, Artist records
            // 3. Link Item to Expression

            // For now, we'll increment count for items we processed
            identified_count += 1;
        }

        Ok(identified_count)
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
        tracing::info!("Starting identification");

        match self.identify_items().await {
            Ok(count) => {
                tracing::info!("Identification complete: {} items processed", count);
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
}
