use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;

pub fn show_status(db_path: PathBuf, _filter: Option<String>) -> Result<()> {
    let db = Database::open(&db_path)?;

    // Get basic statistics
    let unidentified = db.list_unidentified_items()?;
    let unidentified_count = unidentified.len();

    println!("\n📊 Tessitura Status\n");
    println!("  Database: {}", db_path.display());
    println!("  Unidentified items: {}", unidentified_count);

    if unidentified_count > 0 {
        println!("\n  Run `tessitura identify` to identify these items");
    }

    Ok(())
}
