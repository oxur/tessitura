use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tessitura_etl::Config;

mod commands;

#[derive(Debug, Parser)]
#[command(name = "tessitura", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the database (default: ~/.local/share/tessitura/tessitura.db)
    #[arg(long, global = true)]
    db: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Scan a music directory for audio files
    ///
    /// Recursively walks the specified directory to discover audio files and extract
    /// their metadata. For each audio file found:
    ///
    /// - Extracts embedded tags (title, artist, album, track number, year, genre)
    /// - Records file metadata (path, size, format, modification time)
    /// - Creates Item records in the database
    /// - Tracks files in the pipeline for downstream identification
    ///
    /// Supported formats: FLAC, MP3, OGG, WAV, M4A/AAC
    ///
    /// The scan is incremental: previously scanned files are skipped unless their
    /// modification time has changed. Changed files are re-scanned and updated.
    /// Files removed from disk are detected and marked accordingly.
    ///
    /// Output:
    /// - Real-time progress indicators for each stage
    /// - Summary showing files discovered, added, updated, and removed
    /// - No errors for properly tagged files
    ///
    /// Database: Items are stored in the 'items' table with full tag metadata
    /// and provenance tracking. Use 'tessitura status' to view scanned items.
    Scan {
        /// Path to the music directory
        path: PathBuf,
    },
    /// Identify recordings via AcoustID/MusicBrainz
    ///
    /// Processes all unidentified items in the database by matching them against
    /// MusicBrainz recordings. For each unidentified item:
    ///
    /// - Uses AcoustID fingerprint matching (if available)
    /// - Falls back to metadata-based search (artist, album, title)
    /// - Creates Work, Expression, Manifestation, and Artist records
    /// - Links Items to their identified Expressions and Manifestations
    ///
    /// This command only processes items already scanned into the database.
    /// Run 'tessitura scan' first to discover and catalog audio files.
    ///
    /// Requires ACOUSTID_API_KEY environment variable for fingerprint matching.
    /// Rate limits are respected (1 req/sec for MusicBrainz).
    ///
    /// Output:
    /// - Progress for each identification attempt
    /// - Success/failure status per item
    /// - Final summary of identified vs unidentified items
    Identify,
    /// Show pipeline status
    Status {
        /// Optional filter (album name, artist, etc.)
        filter: Option<String>,
    },
}

// Removed: now using Config::load() which has default_db_path internally

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Load configuration from file and environment variables
    let config = if let Some(db_path) = cli.db {
        // CLI flag takes highest priority
        Config::load_with_db_path(db_path)?
    } else {
        Config::load()?
    };

    // Ensure database directory exists
    if let Some(parent) = config.database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match cli.command {
        Commands::Scan { path } => {
            commands::run_scan(path, config.database_path).await?;
        }
        Commands::Identify => {
            commands::run_identify(config.database_path, config.acoustid_api_key).await?;
        }
        Commands::Status { filter } => {
            commands::show_status(config.database_path, filter)?;
        }
    }

    Ok(())
}
