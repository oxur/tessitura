//! Fan-out enrichment stage.
//!
//! Orchestrates concurrent enrichment from multiple external sources
//! (MusicBrainz, Wikidata, Last.fm, Discogs) using treadle's fan-out
//! mechanism.

use std::path::PathBuf;

use treadle::{Stage, StageContext, StageOutcome, SubTask};
use uuid::Uuid;

use crate::config::Config;
use crate::enrich::discogs::DiscogsEnricher;
use crate::enrich::lastfm::LastFmEnricher;
use crate::enrich::musicbrainz::MusicBrainzEnricher;
use crate::enrich::wikidata::WikidataEnricher;
use tessitura_core::model::ItemId;
use tessitura_core::schema::Database;

/// Parse an item ID string into an `ItemId`.
fn parse_item_id(item_id: &str) -> Result<ItemId, treadle::TreadleError> {
    let uuid = Uuid::parse_str(item_id).map_err(|_| {
        treadle::TreadleError::StageExecution(format!("Invalid item ID (not a UUID): {item_id}"))
    })?;
    Ok(ItemId::from_uuid(uuid))
}

/// The Enrich stage: fan-out to multiple metadata sources.
///
/// Each source runs as an independent subtask. If one source fails,
/// the others can still succeed and the failed source can be retried
/// independently.
#[derive(Debug)]
pub struct EnrichStage {
    musicbrainz: Option<MusicBrainzEnricher>,
    wikidata: Option<WikidataEnricher>,
    lastfm: Option<LastFmEnricher>,
    discogs: Option<DiscogsEnricher>,
    db_path: PathBuf,
}

impl EnrichStage {
    /// Create a new `EnrichStage` from configuration.
    ///
    /// Sources are enabled based on available API keys and configuration.
    /// `MusicBrainz` and Wikidata are always available (no API key required).
    pub fn new(config: &Config, db_path: PathBuf) -> Self {
        let musicbrainz = MusicBrainzEnricher::new().ok();
        let wikidata = WikidataEnricher::new().ok();
        let lastfm = config
            .lastfm_api_key
            .as_ref()
            .map(|key| LastFmEnricher::new(key.clone()));
        let discogs = Some(DiscogsEnricher::new(config.discogs_token.clone()));

        Self {
            musicbrainz,
            wikidata,
            lastfm,
            discogs,
            db_path,
        }
    }

    /// List which enrichment sources are enabled.
    #[must_use]
    pub fn enabled_sources(&self) -> Vec<&str> {
        let mut sources = Vec::new();
        if self.musicbrainz.is_some() {
            sources.push("musicbrainz");
        }
        if self.wikidata.is_some() {
            sources.push("wikidata");
        }
        if self.lastfm.is_some() {
            sources.push("lastfm");
        }
        if self.discogs.is_some() {
            sources.push("discogs");
        }
        sources
    }

    async fn enrich_from_musicbrainz(&self, item_id: &str) -> Result<(), treadle::TreadleError> {
        let enricher = self.musicbrainz.as_ref().ok_or_else(|| {
            treadle::TreadleError::StageExecution("MusicBrainz enricher not available".to_string())
        })?;

        // Read phase: open DB, extract needed data, then drop DB before async work.
        let recording_mbid = {
            let db = Database::open(&self.db_path).map_err(|e| {
                treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
            })?;

            let item_id_parsed = parse_item_id(item_id)?;
            let item = db
                .get_item_by_id(&item_id_parsed)
                .map_err(|e| {
                    treadle::TreadleError::StageExecution(format!("Failed to get item: {e}"))
                })?
                .ok_or_else(|| {
                    treadle::TreadleError::StageExecution(format!("Item not found: {item_id}"))
                })?;

            item.expression_id.and_then(|expr_id| {
                let expressions = db.list_expressions().ok()?;
                expressions
                    .into_iter()
                    .find(|e| e.id == expr_id)
                    .and_then(|expr| expr.musicbrainz_id)
            })
        };
        // `db` is now dropped -- safe for Send futures.

        if let Some(ref mbid) = recording_mbid {
            match enricher
                .enrich_recording(mbid, item_id, &self.db_path)
                .await
            {
                Ok(assertions) => {
                    log::info!(
                        "MusicBrainz enrichment: {} assertions for {}",
                        assertions.len(),
                        item_id
                    );
                }
                Err(e) => {
                    log::warn!("MusicBrainz enrichment failed for {}: {}", item_id, e);
                }
            }
        }

        Ok(())
    }

    async fn enrich_from_wikidata(&self, item_id: &str) -> Result<(), treadle::TreadleError> {
        let enricher = self.wikidata.as_ref().ok_or_else(|| {
            treadle::TreadleError::StageExecution("Wikidata enricher not available".to_string())
        })?;

        // Read phase: open DB, extract needed data, then drop DB before async work.
        let work_mbid = {
            let db = Database::open(&self.db_path).map_err(|e| {
                treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
            })?;

            let item_id_parsed = parse_item_id(item_id)?;
            let item = db
                .get_item_by_id(&item_id_parsed)
                .map_err(|e| {
                    treadle::TreadleError::StageExecution(format!("Failed to get item: {e}"))
                })?
                .ok_or_else(|| {
                    treadle::TreadleError::StageExecution(format!("Item not found: {item_id}"))
                })?;

            item.expression_id.and_then(|expr_id| {
                let expressions = db.list_expressions().ok()?;
                let expr = expressions.into_iter().find(|e| e.id == expr_id)?;
                db.get_work_by_musicbrainz_id(&expr.work_id.to_string())
                    .ok()
                    .flatten()
                    .and_then(|w| w.musicbrainz_id)
            })
        };
        // `db` is now dropped -- safe for Send futures.

        if let Some(ref mbid) = work_mbid {
            match enricher.enrich(mbid, item_id, &self.db_path).await {
                Ok(assertions) => {
                    log::info!(
                        "Wikidata enrichment: {} assertions for {}",
                        assertions.len(),
                        item_id
                    );
                }
                Err(e) => {
                    log::warn!("Wikidata enrichment failed for {}: {}", item_id, e);
                }
            }
        }

        Ok(())
    }

    async fn enrich_from_lastfm(&self, item_id: &str) -> Result<(), treadle::TreadleError> {
        let enricher = self.lastfm.as_ref().ok_or_else(|| {
            treadle::TreadleError::StageExecution("Last.fm enricher not available".to_string())
        })?;

        // Read phase: open DB, extract needed data, then drop DB before async work.
        let artist_and_title = {
            let db = Database::open(&self.db_path).map_err(|e| {
                treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
            })?;

            let item_id_parsed = parse_item_id(item_id)?;
            let item = db
                .get_item_by_id(&item_id_parsed)
                .map_err(|e| {
                    treadle::TreadleError::StageExecution(format!("Failed to get item: {e}"))
                })?
                .ok_or_else(|| {
                    treadle::TreadleError::StageExecution(format!("Item not found: {item_id}"))
                })?;

            item.tag_artist.zip(item.tag_title)
        };
        // `db` is now dropped -- safe for Send futures.

        if let Some((artist, title)) = artist_and_title {
            match enricher
                .enrich(&artist, &title, item_id, &self.db_path)
                .await
            {
                Ok(assertions) => {
                    log::info!(
                        "Last.fm enrichment: {} assertions for {}",
                        assertions.len(),
                        item_id
                    );
                }
                Err(e) => {
                    log::warn!("Last.fm enrichment failed for {}: {}", item_id, e);
                }
            }
        } else {
            log::debug!(
                "Skipping Last.fm enrichment for {} (no artist/title tags)",
                item_id
            );
        }

        Ok(())
    }

    async fn enrich_from_discogs(&self, item_id: &str) -> Result<(), treadle::TreadleError> {
        let enricher = self.discogs.as_ref().ok_or_else(|| {
            treadle::TreadleError::StageExecution("Discogs enricher not available".to_string())
        })?;

        // Read phase: open DB, extract needed data, then drop DB before async work.
        let catalog_number = {
            let db = Database::open(&self.db_path).map_err(|e| {
                treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
            })?;

            let item_id_parsed = parse_item_id(item_id)?;
            let item = db
                .get_item_by_id(&item_id_parsed)
                .map_err(|e| {
                    treadle::TreadleError::StageExecution(format!("Failed to get item: {e}"))
                })?
                .ok_or_else(|| {
                    treadle::TreadleError::StageExecution(format!("Item not found: {item_id}"))
                })?;

            item.manifestation_id.and_then(|man_id| {
                db.get_manifestation_by_musicbrainz_id(&man_id.to_string())
                    .ok()
                    .flatten()
                    .and_then(|man| man.catalog_number)
            })
        };
        // `db` is now dropped -- safe for Send futures.

        if let Some(ref catno) = catalog_number {
            match enricher
                .enrich_by_catno(catno, item_id, &self.db_path)
                .await
            {
                Ok(assertions) => {
                    log::info!(
                        "Discogs enrichment: {} assertions for {}",
                        assertions.len(),
                        item_id
                    );
                }
                Err(e) => {
                    log::warn!("Discogs enrichment failed for {}: {}", item_id, e);
                }
            }
        } else {
            log::debug!(
                "Skipping Discogs enrichment for {} (no catalog number)",
                item_id
            );
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Stage for EnrichStage {
    fn name(&self) -> &str {
        "enrich"
    }

    async fn execute(
        &self,
        item: &dyn treadle::WorkItem,
        ctx: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        match ctx.subtask_name.as_deref() {
            // First call: fan out to all enabled sources
            None => {
                let mut subtasks = Vec::new();
                if self.musicbrainz.is_some() {
                    subtasks.push(SubTask::new("musicbrainz".to_string()));
                }
                if self.wikidata.is_some() {
                    subtasks.push(SubTask::new("wikidata".to_string()));
                }
                if self.lastfm.is_some() {
                    subtasks.push(SubTask::new("lastfm".to_string()));
                }
                if self.discogs.is_some() {
                    subtasks.push(SubTask::new("discogs".to_string()));
                }

                if subtasks.is_empty() {
                    log::warn!("No enrichment sources enabled");
                    return Ok(StageOutcome::Complete);
                }

                log::info!(
                    "Enriching {} with {} sources: {:?}",
                    item.id(),
                    subtasks.len(),
                    self.enabled_sources()
                );

                Ok(StageOutcome::FanOut(subtasks))
            }

            // Subtask dispatching
            Some("musicbrainz") => {
                self.enrich_from_musicbrainz(item.id()).await?;
                Ok(StageOutcome::Complete)
            }
            Some("wikidata") => {
                self.enrich_from_wikidata(item.id()).await?;
                Ok(StageOutcome::Complete)
            }
            Some("lastfm") => {
                self.enrich_from_lastfm(item.id()).await?;
                Ok(StageOutcome::Complete)
            }
            Some("discogs") => {
                self.enrich_from_discogs(item.id()).await?;
                Ok(StageOutcome::Complete)
            }

            Some(other) => Err(treadle::TreadleError::StageExecution(format!(
                "Unknown enrichment subtask: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_enrich_stage_creation() {
        let config = test_config();
        let stage = EnrichStage::new(&config, PathBuf::from("/tmp/test.db"));
        assert_eq!(stage.name(), "enrich");
    }

    #[test]
    fn test_enrich_stage_enabled_sources_default() {
        let config = test_config();
        let stage = EnrichStage::new(&config, PathBuf::from("/tmp/test.db"));
        let sources = stage.enabled_sources();
        // MusicBrainz, Wikidata, and Discogs are always available
        assert!(sources.contains(&"musicbrainz"));
        assert!(sources.contains(&"wikidata"));
        assert!(sources.contains(&"discogs"));
        // Last.fm requires an API key
        assert!(!sources.contains(&"lastfm"));
    }

    #[test]
    fn test_enrich_stage_with_lastfm_key() {
        let mut config = test_config();
        config.lastfm_api_key = Some("test-key".to_string());
        let stage = EnrichStage::new(&config, PathBuf::from("/tmp/test.db"));
        let sources = stage.enabled_sources();
        assert!(sources.contains(&"lastfm"));
    }

    #[tokio::test]
    async fn test_enrich_stage_fan_out() {
        let config = test_config();
        let stage = EnrichStage::new(&config, PathBuf::from("/tmp/test.db"));

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestItem {
            id: String,
        }

        impl treadle::WorkItem for TestItem {
            fn id(&self) -> &str {
                &self.id
            }
        }

        let item = TestItem {
            id: "test-item".to_string(),
        };
        let mut ctx = StageContext::new("enrich".to_string());

        let outcome = stage.execute(&item, &mut ctx).await.unwrap();
        match outcome {
            StageOutcome::FanOut(subtasks) => {
                assert!(!subtasks.is_empty());
                let ids: Vec<&str> = subtasks.iter().map(|s| s.id.as_str()).collect();
                assert!(ids.contains(&"musicbrainz"));
                assert!(ids.contains(&"wikidata"));
                assert!(ids.contains(&"discogs"));
            }
            _ => panic!("Expected FanOut outcome"),
        }
    }

    #[tokio::test]
    async fn test_enrich_stage_unknown_subtask() {
        let config = test_config();
        let stage = EnrichStage::new(&config, PathBuf::from("/tmp/test.db"));

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestItem {
            id: String,
        }

        impl treadle::WorkItem for TestItem {
            fn id(&self) -> &str {
                &self.id
            }
        }

        let item = TestItem {
            id: "test-item".to_string(),
        };
        let mut ctx = StageContext::new("enrich".to_string()).with_subtask("unknown_source");

        let result = stage.execute(&item, &mut ctx).await;
        assert!(result.is_err());
    }
}
