use std::path::PathBuf;

use anyhow::{Context, Result};
use tessitura_core::schema::Database;
use tessitura_etl::enrich::lcgft;

/// Load LCGFT and/or LCMPT vocabulary snapshots into the database.
pub fn load_vocab(
    db_path: PathBuf,
    lcgft_path: Option<PathBuf>,
    lcmpt_path: Option<PathBuf>,
) -> Result<()> {
    let db = Database::open(&db_path).context("Failed to open database")?;

    if let Some(path) = &lcgft_path {
        let count = lcgft::load_lcgft(&db, path)?;
        println!("Loaded {count} LCGFT terms from {}", path.display());
    }

    if let Some(path) = &lcmpt_path {
        let count = lcgft::load_lcmpt(&db, path)?;
        println!("Loaded {count} LCMPT terms from {}", path.display());
    }

    if lcgft_path.is_none() && lcmpt_path.is_none() {
        // Load from default locations
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tessitura");

        let lcgft_default = config_dir.join("lcgft-snapshot.json");
        let lcmpt_default = config_dir.join("lcmpt-snapshot.json");

        let mut loaded = false;

        if lcgft_default.exists() {
            let count = lcgft::load_lcgft(&db, &lcgft_default)?;
            println!(
                "Loaded {count} LCGFT terms from {}",
                lcgft_default.display()
            );
            loaded = true;
        }

        if lcmpt_default.exists() {
            let count = lcgft::load_lcmpt(&db, &lcmpt_default)?;
            println!(
                "Loaded {count} LCMPT terms from {}",
                lcmpt_default.display()
            );
            loaded = true;
        }

        if !loaded {
            println!("No vocabulary snapshots found.");
            println!("Place snapshot files at:");
            println!("  LCGFT: {}", lcgft_default.display());
            println!("  LCMPT: {}", lcmpt_default.display());
            println!();
            println!("Or specify paths explicitly:");
            println!(
                "  tessitura vocab load --lcgft /path/to/lcgft.json --lcmpt /path/to/lcmpt.json"
            );
        }
    }

    Ok(())
}

/// Show vocabulary statistics.
pub fn vocab_stats(db_path: PathBuf) -> Result<()> {
    let db = Database::open(&db_path).context("Failed to open database")?;

    let lcgft_count = db.count_lcgft_terms()?;
    let lcmpt_count = db.count_lcmpt_terms()?;

    println!("Vocabulary Statistics:");
    println!("  LCGFT (Genre/Form Terms): {lcgft_count}");
    println!("  LCMPT (Medium of Performance Terms): {lcmpt_count}");

    if lcgft_count == 0 && lcmpt_count == 0 {
        println!();
        println!("No vocabulary data loaded. Run 'tessitura vocab load' to import snapshots.");
    }

    Ok(())
}
