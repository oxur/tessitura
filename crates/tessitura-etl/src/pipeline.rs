use std::path::PathBuf;
use treadle::Workflow;

use crate::config::Config;
use crate::{EnrichStage, HarmonizeStage, IdentifyStage, ScanStage};

/// Build the scan + identify pipeline.
///
/// # Errors
/// Returns an error if the workflow cannot be built.
pub fn build_pipeline(
    music_dir: PathBuf,
    db_path: PathBuf,
    acoustid_api_key: Option<String>,
) -> treadle::Result<Workflow> {
    let scan_stage = ScanStage::new(music_dir, db_path.clone());
    let identify_stage = IdentifyStage::new(acoustid_api_key, db_path).map_err(|e| {
        treadle::TreadleError::InvalidWorkflow(format!("Failed to create identify stage: {e}"))
    })?;

    Workflow::builder()
        .stage("scan", scan_stage)
        .stage("identify", identify_stage)
        .dependency("identify", "scan")
        .build()
}

/// Build the full 4-stage pipeline: scan → identify → enrich → harmonize.
///
/// # Errors
/// Returns an error if the workflow or any stage cannot be built.
pub fn build_full_pipeline(
    music_dir: PathBuf,
    db_path: PathBuf,
    config: &Config,
) -> treadle::Result<Workflow> {
    let scan_stage = ScanStage::new(music_dir, db_path.clone());
    let identify_stage = IdentifyStage::new(config.acoustid_api_key.clone(), db_path.clone())
        .map_err(|e| {
            treadle::TreadleError::InvalidWorkflow(format!("Failed to create identify stage: {e}"))
        })?;
    let enrich_stage = EnrichStage::new(config, db_path.clone());
    let harmonize_stage = HarmonizeStage::new(&config.rules_path, db_path).map_err(|e| {
        treadle::TreadleError::InvalidWorkflow(format!("Failed to create harmonize stage: {e}"))
    })?;

    Workflow::builder()
        .stage("scan", scan_stage)
        .stage("identify", identify_stage)
        .stage("enrich", enrich_stage)
        .stage("harmonize", harmonize_stage)
        .dependency("identify", "scan")
        .dependency("enrich", "identify")
        .dependency("harmonize", "enrich")
        .build()
}
