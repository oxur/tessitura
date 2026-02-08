use std::path::PathBuf;

use anyhow::Result;

/// Run the review TUI for human review of proposed metadata.
pub fn run_review(db_path: PathBuf) -> Result<()> {
    crate::tui::run_tui(db_path)
}
