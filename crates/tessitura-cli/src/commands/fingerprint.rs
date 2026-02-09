use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::audio::generate_fingerprint;

pub async fn run_fingerprint(db_path: PathBuf, force: bool) -> Result<()> {
    log::info!("Starting fingerprint generation");

    let db = Database::open(&db_path)?;

    // Get items to fingerprint
    let items = if force {
        log::info!("Force mode: re-fingerprinting all items");
        db.list_all_items()?
    } else {
        log::info!("Normal mode: fingerprinting items without fingerprints");
        db.list_items_without_fingerprints()?
    };

    if items.is_empty() {
        println!("No items to fingerprint");
        return Ok(());
    }

    println!("Found {} items to fingerprint", items.len());
    println!("This may take a while...\n");

    let mut success_count = 0;
    let mut failure_count = 0;
    let mut skipped_count = 0;

    for (idx, item) in items.iter().enumerate() {
        let progress = format!("[{}/{}]", idx + 1, items.len());

        // Skip 0-byte files (Dropbox placeholders, corrupted files, etc.)
        if item.file_size == 0 {
            log::debug!(
                "{} Skipping 0-byte file: {}",
                progress,
                item.file_path.display()
            );
            skipped_count += 1;
            continue;
        }

        log::debug!(
            "{} Fingerprinting: {}",
            progress,
            item.file_path.display()
        );
        print!("\r{} Processing: {}", progress, item.file_path.display());
        std::io::Write::flush(&mut std::io::stdout())?;

        match generate_fingerprint(&item.file_path) {
            Ok((fingerprint, duration)) => {
                // Update item with fingerprint
                let mut updated_item = item.clone();
                updated_item.fingerprint = Some(fingerprint.clone());

                // Update duration if we got a more accurate one from decoding
                if item.duration_secs.is_none() || force {
                    updated_item.duration_secs = Some(duration);
                }

                match db.update_item(&updated_item) {
                    Ok(()) => {
                        log::debug!(
                            "{} ✓ Fingerprint stored ({}s)",
                            progress,
                            duration
                        );
                        success_count += 1;
                    }
                    Err(e) => {
                        log::error!(
                            "{} Failed to update item {}: {}",
                            progress,
                            item.file_path.display(),
                            e
                        );
                        failure_count += 1;
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "{} Failed to generate fingerprint for {}: {}",
                    progress,
                    item.file_path.display(),
                    e
                );
                failure_count += 1;
            }
        }
    }

    println!("\r"); // Clear progress line
    println!("\n✓ Fingerprinting complete");
    println!("  Successful: {}", success_count);
    println!("  Skipped:    {} (0-byte files)", skipped_count);
    println!("  Failed:     {}", failure_count);

    if skipped_count > 0 {
        println!(
            "\nNote: {} files were skipped (0 bytes - likely Dropbox placeholders or corrupted).",
            skipped_count
        );
    }

    if failure_count > 0 {
        println!(
            "\nNote: {} files failed fingerprinting. Check logs for details.",
            failure_count
        );
    }

    Ok(())
}
