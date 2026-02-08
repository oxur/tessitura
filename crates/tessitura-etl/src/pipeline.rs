use std::path::PathBuf;
use treadle::Workflow;

use crate::{IdentifyStage, ScanStage};

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
