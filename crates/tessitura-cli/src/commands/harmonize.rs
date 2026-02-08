use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;

pub fn run_harmonize(db_path: PathBuf, rules_path: PathBuf) -> Result<()> {
    log::info!("Starting harmonization");

    let db = Database::open(&db_path)?;

    // Check for assertions that need harmonizing
    // For now, count total assertions as a proxy
    let items = db.list_identified_items()?;

    if items.is_empty() {
        println!("No identified items in database. Run 'tessitura identify' first.");
        return Ok(());
    }

    // Verify rules file exists
    if !rules_path.exists() {
        println!("Mapping rules file not found: {}", rules_path.display());
        println!("\nTo get started:");
        println!(
            "  1. Copy the default rules: cp config/taxonomy.toml {}",
            rules_path.display()
        );
        println!("  2. Or set a custom path: TESS_RULES_PATH=/path/to/rules.toml");
        return Ok(());
    }

    println!("Using mapping rules: {}", rules_path.display());
    println!("{} identified items ready for harmonization", items.len());

    // TODO: Wire harmonization execution through treadle Workflow
    println!("\nHarmonization pipeline ready. Full execution coming in next iteration.");

    Ok(())
}
