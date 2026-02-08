use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::{build_pipeline, MusicFile};

pub async fn run_identify(
    music_dir: PathBuf,
    db_path: PathBuf,
    acoustid_api_key: Option<String>,
) -> Result<()> {
    tracing::info!("Starting identification");

    // Check how many unidentified items we have
    let db = Database::open(&db_path)?;
    let unidentified = db.list_unidentified_items()?;
    println!("Found {} unidentified items", unidentified.len());

    if unidentified.is_empty() {
        println!("No items to identify");
        return Ok(());
    }

    // Build the full pipeline
    let workflow = build_pipeline(music_dir.clone(), db_path.clone(), acoustid_api_key)?;

    // Create a state store
    let state_path = db_path.parent().unwrap().join("pipeline.db");
    let mut store = treadle::SqliteStateStore::open(&state_path).await?;

    // Create a work item
    let identify_job = MusicFile::new("identify-job", music_dir.clone());

    // Subscribe to events
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

    // Execute
    workflow.advance(&identify_job, &mut store).await?;

    println!("\n✓ Identification complete");
    Ok(())
}
