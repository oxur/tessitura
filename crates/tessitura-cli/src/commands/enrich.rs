use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::Config;

pub async fn run_enrich(config: &Config, db_path: PathBuf, pending_only: bool) -> Result<()> {
    log::info!("Starting enrichment");

    let db = Database::open(&db_path)?;

    // Count items awaiting enrichment
    let identified_items = db.list_identified_items()?;

    if identified_items.is_empty() {
        println!("No identified items in database. Run 'tessitura identify' first.");
        return Ok(());
    }

    let enrich_stage = tessitura_etl::EnrichStage::new(config, db_path);
    let sources = enrich_stage.enabled_sources();

    println!(
        "Enrichment sources enabled: {}",
        if sources.is_empty() {
            "none".to_string()
        } else {
            sources.join(", ")
        }
    );

    if sources.is_empty() {
        println!("\nNo enrichment sources configured.");
        println!("Set API keys in config or environment variables:");
        println!("  TESS_LASTFM_API_KEY   - Last.fm folksonomy tags");
        println!("  TESS_DISCOGS_TOKEN    - Discogs release details");
        println!("(MusicBrainz and Wikidata require no API key)");
        return Ok(());
    }

    println!(
        "{} identified items ready for enrichment{}",
        identified_items.len(),
        if pending_only { " (pending only)" } else { "" }
    );

    // TODO: Wire enrichment execution through treadle Workflow
    // For now, show what would be enriched
    println!("\nEnrichment pipeline ready. Full execution coming in next iteration.");
    println!("Sources: {:?} | Items: {}", sources, identified_items.len());

    Ok(())
}
