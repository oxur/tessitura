use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::identify::IdentifyStage;
use tessitura_etl::MusicFile;
use treadle::Stage;

pub async fn run_identify(db_path: PathBuf, acoustid_api_key: Option<String>) -> Result<()> {
    log::info!("Starting identification");

    // Check how many unidentified items we have
    let db = Database::open(&db_path)?;
    let unidentified = db.list_unidentified_items()?;

    println!(
        "Found {} unidentified items in database",
        unidentified.len()
    );

    if unidentified.is_empty() {
        println!("No items to identify");
        return Ok(());
    }

    // Create the identify stage and run it
    let stage = IdentifyStage::new(acoustid_api_key, db_path)
        .map_err(|e| anyhow::anyhow!("Failed to create IdentifyStage: {}", e))?;

    println!("\nStarting identification process...");
    println!("This may take a while for {} items...", unidentified.len());

    // Use a dummy work item - the stage operates in batch mode
    let item = MusicFile::new(
        "batch-identify",
        std::path::PathBuf::from("/tmp/batch-identify"),
    );
    let mut ctx = treadle::StageContext::new("identify".to_string());

    stage
        .execute(&item, &mut ctx)
        .await
        .map_err(|e| anyhow::anyhow!("Identification failed: {}", e))?;

    println!("\n✓ Identification complete");
    println!("Run 'tessitura status' to see identified items");

    Ok(())
}
