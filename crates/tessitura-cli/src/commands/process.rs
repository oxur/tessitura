use anyhow::{Context, Result};
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::{build_full_pipeline, Config, MusicFile};

/// Orchestrate the complete processing pipeline.
///
/// Steps:
/// 1. Scan - discover audio files and extract metadata
/// 2. Fingerprint - generate acoustic fingerprints
/// 3. Identify - match to MusicBrainz recordings
/// 4. Enrich - fetch metadata from external sources
/// 5. Harmonize - apply mapping rules and resolve conflicts
pub async fn run_process(
    music_dir: PathBuf,
    db_path: PathBuf,
    config: &Config,
    resume: bool,
) -> Result<()> {
    println!("\n🎵 Tessitura Full Processing Pipeline\n");
    println!("  Music directory: {}", music_dir.display());
    println!("  Database: {}", db_path.display());
    println!();

    // Track which steps have been completed
    let mut completed_steps: Vec<&str> = Vec::new();

    // Step 1: Scan (unless resuming and already complete)
    if !resume || should_run_scan(&db_path)? {
        println!("📁 Step 1/5: Scanning music directory...");
        super::run_scan(music_dir.clone(), db_path.clone())
            .await
            .context("Scan step failed")?;
        completed_steps.push("scan");
        println!("  ✓ Scan complete\n");
    } else {
        println!("📁 Step 1/5: Scan (skipped - already complete)\n");
    }

    // Step 2: Fingerprint (unless resuming and already complete)
    if !resume || should_run_fingerprint(&db_path)? {
        println!("🎵 Step 2/5: Generating acoustic fingerprints...");
        super::run_fingerprint(db_path.clone(), false)
            .await
            .context("Fingerprint step failed")?;
        completed_steps.push("fingerprint");
        println!("  ✓ Fingerprint complete\n");
    } else {
        println!("🎵 Step 2/5: Fingerprint (skipped - already complete)\n");
    }

    // Step 3: Identify (unless resuming and already complete)
    if !resume || should_run_identify(&db_path)? {
        println!("🔍 Step 3/5: Identifying recordings...");
        super::run_identify(db_path.clone(), config.acoustid_api_key.clone())
            .await
            .context("Identify step failed")?;
        completed_steps.push("identify");
        println!("  ✓ Identify complete\n");
    } else {
        println!("🔍 Step 3/5: Identify (skipped - already complete)\n");
    }

    // Steps 4-5: Build and run the full treadle pipeline
    // (enrich + harmonize are treadle stages)
    println!("📚 Step 4/5: Enriching metadata from external sources...");
    println!("⚖️  Step 5/5: Harmonizing and resolving conflicts...");

    let workflow = build_full_pipeline(music_dir.clone(), db_path.clone(), config)
        .context("Failed to build pipeline")?;

    // Create state store
    let parent = db_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Database path has no parent directory"))?;
    let state_path = parent.join("pipeline.db");
    let mut store = treadle::SqliteStateStore::open(&state_path)
        .await
        .context("Failed to open pipeline state store")?;

    // Create work item
    let work_item = MusicFile::new("process-job", music_dir);

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
    workflow
        .advance(&work_item, &mut store)
        .await
        .context("Pipeline execution failed")?;

    println!("\n✓ Full processing pipeline complete!");
    println!("\nNext steps:");
    println!("  - Run 'tessitura review' to review and approve proposed metadata");
    println!("  - Run 'tessitura status' to see pipeline status");

    Ok(())
}

/// Check if scan should run (no items in database).
fn should_run_scan(db_path: &PathBuf) -> Result<bool> {
    if !db_path.exists() {
        return Ok(true);
    }
    let db = Database::open(db_path)?;
    let items = db.list_all_items()?;
    Ok(items.is_empty())
}

/// Check if fingerprint should run (items exist without fingerprints).
fn should_run_fingerprint(db_path: &PathBuf) -> Result<bool> {
    let db = Database::open(db_path)?;
    let items = db.list_items_without_fingerprints()?;
    Ok(!items.is_empty())
}

/// Check if identify should run (unidentified items exist).
fn should_run_identify(db_path: &PathBuf) -> Result<bool> {
    let db = Database::open(db_path)?;
    let items = db.list_unidentified_items()?;
    Ok(!items.is_empty())
}
