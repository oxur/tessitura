use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;

pub fn show_status(db_path: PathBuf, _filter: Option<String>) -> Result<()> {
    let db = Database::open(&db_path)?;

    // Get statistics for each pipeline step
    let total_items = db.list_all_items()?.len();
    let items_without_fingerprints = db.list_items_without_fingerprints()?.len();
    let unidentified_items = db.list_unidentified_items()?.len();
    let identified_items = db.list_identified_items()?.len();

    println!("\n📊 Tessitura Status\n");
    println!("  Database: {}", db_path.display());
    println!();
    println!("Pipeline Progress:");
    println!("  ✓ Scan:        {} items", total_items);
    println!(
        "  {} Fingerprint: {} items {}",
        if items_without_fingerprints == 0 {
            "✓"
        } else {
            "⏳"
        },
        total_items - items_without_fingerprints,
        if items_without_fingerprints > 0 {
            format!("({} pending)", items_without_fingerprints)
        } else {
            String::new()
        }
    );
    println!(
        "  {} Identify:    {} items {}",
        if unidentified_items == 0 { "✓" } else { "⏳" },
        identified_items,
        if unidentified_items > 0 {
            format!("({} pending)", unidentified_items)
        } else {
            String::new()
        }
    );
    // TODO: Add enrich and harmonize status when those stages track completion

    if total_items == 0 {
        println!("\nNo items found. Run 'tessitura process <dir>' to scan and process your music library.");
    } else if items_without_fingerprints > 0 {
        println!("\nNext step: Run 'tessitura process --resume' to continue processing");
    } else if unidentified_items > 0 {
        println!("\nNext step: Run 'tessitura process --resume' to continue processing");
    } else if identified_items > 0 {
        println!("\nNext step: Run 'tessitura enrich' to fetch external metadata");
    }

    Ok(())
}
