use anyhow::Result;
use std::path::PathBuf;
use tessitura_etl::{build_pipeline, MusicFile};

pub async fn run_scan(music_dir: PathBuf, db_path: PathBuf) -> Result<()> {
    tracing::info!("Starting scan of {}", music_dir.display());

    // Build the pipeline (just scan stage for now)
    let workflow = build_pipeline(music_dir.clone(), db_path.clone(), None)?;

    // Create a state store for the pipeline
    let state_path = db_path.parent().unwrap().join("pipeline.db");
    let mut store = treadle::SqliteStateStore::open(&state_path).await?;

    // Create a work item for the scan job
    let scan_job = MusicFile::new("scan-job", music_dir.clone());

    // Subscribe to events for progress display
    let mut events = workflow.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                treadle::WorkflowEvent::StageStarted { stage, .. } => {
                    println!("  ⏳ [{stage}] Starting...");
                }
                treadle::WorkflowEvent::StageCompleted { stage, .. } => {
                    println!("  ✓ [{stage}] Complete");
                }
                treadle::WorkflowEvent::StageFailed { stage, error, .. } => {
                    eprintln!("  ✗ [{stage}] FAILED: {error}");
                }
                _ => {}
            }
        }
    });

    // Execute the workflow
    workflow.advance(&scan_job, &mut store).await?;

    println!("\n✓ Scan complete");
    Ok(())
}
