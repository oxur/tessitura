use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tessitura_etl::Config;

mod commands;
mod tui;

#[derive(Debug, Parser)]
#[command(name = "tessitura", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the database (default: ~/.local/share/tessitura/library.db)
    #[arg(long, global = true)]
    db: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Process a music library through the full pipeline
    #[command(
        long_about = "Orchestrates the complete metadata processing pipeline from start to finish:

  1. Scan - Discover audio files and extract embedded metadata
  2. Fingerprint - Generate acoustic fingerprints for identification
  3. Identify - Match recordings to MusicBrainz database
  4. Enrich - Fetch metadata from external sources (Wikidata, Last.fm, Discogs)
  5. Harmonize - Apply mapping rules and resolve conflicts

This command stops on failure and reports which step failed. Use --resume to
skip already-completed steps when restarting after a failure.

The process command is the recommended way to initialize a new music library
in Tessitura. After completion, use 'tessitura review' to approve the
proposed metadata changes.

Output:
  - Progress indicators for each step
  - Clear error messages indicating which step failed
  - Summary statistics at completion"
    )]
    Process {
        /// Path to the music directory
        path: PathBuf,

        /// Resume from last successful step (skip completed steps)
        #[arg(long, short, default_value_t = false)]
        resume: bool,
    },
    /// Scan a music directory for audio files
    #[command(
        long_about = "Recursively walks the specified directory to discover audio files and extract
their metadata. For each audio file found:

  - Extracts embedded tags (title, artist, album, track number, year, genre)
  - Records file metadata (path, size, format, modification time)
  - Creates Item records in the database
  - Tracks files in the pipeline for downstream identification

Supported formats: FLAC, MP3, OGG, WAV, M4A/AAC

The scan is incremental: previously scanned files are skipped unless their
modification time has changed. Changed files are re-scanned and updated.
Files removed from disk are detected and marked accordingly.

Output:
  - Real-time progress indicators for each stage
  - Summary showing files discovered, added, updated, and removed
  - No errors for properly tagged files

Database: Items are stored in the 'items' table with full tag metadata
and provenance tracking. Use 'tessitura status' to view scanned items."
    )]
    Scan {
        /// Path to the music directory
        path: PathBuf,
    },
    /// Identify recordings via AcoustID/MusicBrainz
    #[command(alias = "id")]
    #[command(
        long_about = "Processes all unidentified items in the database by matching them against
MusicBrainz recordings. For each unidentified item:

  - Uses AcoustID fingerprint matching (if available)
  - Falls back to metadata-based search (artist, album, title)
  - Creates Work, Expression, Manifestation, and Artist records
  - Links Items to their identified Expressions and Manifestations

This command only processes items already scanned into the database.
Run 'tessitura scan' first to discover and catalog audio files.

Requires TESS_ACOUSTID_API_KEY environment variable for fingerprint matching.
Rate limits are respected (1 req/sec for MusicBrainz).

Output:
  - Progress for each identification attempt
  - Success/failure status per item
  - Final summary of identified vs unidentified items"
    )]
    Identify,
    /// Generate acoustic fingerprints for items
    #[command(
        long_about = "Generates acoustic fingerprints for audio files that don't have them.
This is useful for backfilling fingerprints after initial scanning, or when
fingerprinting was disabled during scan.

Process:
  - Decodes audio to mono PCM at 11025 Hz (Chromaprint standard)
  - Generates Chromaprint fingerprint
  - Stores fingerprint in database for later identification
  - Updates duration if more accurate than tag metadata

By default, only processes items where fingerprint IS NULL.
Use --force to re-fingerprint all items (useful after algorithm updates).

Rate: Processes one file at a time. Large collections may take time."
    )]
    Fingerprint {
        /// Re-fingerprint all items, not just those without fingerprints
        #[arg(long, short, default_value_t = false)]
        force: bool,
    },
    /// Enrich identified items from external metadata sources
    #[command(
        long_about = "Fetches metadata for identified items from multiple external sources:

  - MusicBrainz: recording, work, and release details (no API key needed)
  - Wikidata: key, form, instrumentation, period (no API key needed)
  - Last.fm: folksonomy tags (requires TESS_LASTFM_API_KEY)
  - Discogs: label, format, personnel (optional TESS_DISCOGS_TOKEN)

Each source runs as an independent subtask. If one source fails, the
others can still succeed and the failed source can be retried independently.

Items must be identified first via 'tessitura identify'.

Rate limits are respected per source. All findings are stored as
provenance-tracked assertions in the database."
    )]
    Enrich {
        /// Only enrich items that haven't been enriched yet
        #[arg(long, default_value_t = false)]
        pending_only: bool,
    },
    /// Apply mapping rules and resolve conflicts
    #[command(alias = "harmonise")]
    #[command(
        long_about = "Applies the mapping rules engine to enrichment assertions, normalizing
raw metadata into canonical genre, form, period, and instrumentation values.

The harmonization process:
  1. Loads all assertions for each enriched item
  2. Applies genre rules (pattern matching with source filtering)
  3. Applies period rules (composer name + composition year)
  4. Applies instrument rules (string matching to standard terms)
  5. Resolves conflicts using source priority ordering
  6. Produces proposed tags for human review

Mapping rules are loaded from the taxonomy.toml file (see config).

After harmonization, use 'tessitura review' to approve proposed tags."
    )]
    Harmonize,
    /// Review proposed metadata in a terminal UI
    #[command(
        long_about = "Opens an interactive terminal UI for reviewing proposed metadata
after harmonization. Albums are grouped by name, and each track shows
its proposed tags with source and confidence information.

The review TUI supports:
  - Album list view: browse all albums awaiting review
  - Track detail view: inspect proposed tags for each track
  - Keyboard navigation: j/k or arrow keys, Enter to select, b to go back

This step is intended for human verification before tags are written
back to audio files. Items must be harmonized first via 'tessitura harmonize'."
    )]
    Review,
    /// Show pipeline status
    Status {
        /// Optional filter (album name, artist, etc.)
        filter: Option<String>,
    },
    /// Manage controlled vocabularies (LCGFT/LCMPT)
    #[command(
        long_about = "Load and manage Library of Congress controlled vocabularies used
for genre/form classification (LCGFT) and instrumentation (LCMPT).

These vocabularies provide standardized terms for the mapping rules engine
used during harmonization. They are loaded from JSON snapshot files.

Examples:
  tessitura vocab load                           # Load from default locations
  tessitura vocab load --lcgft /path/to/lcgft.json  # Load specific LCGFT file
  tessitura vocab stats                          # Show vocabulary statistics"
    )]
    Vocab {
        #[command(subcommand)]
        action: VocabAction,
    },
    /// Manage mapping rules
    #[command(long_about = "Manage genre, period, and instrumentation mapping rules.

Rules file location:
  Linux:   ~/.config/tessitura/taxonomy.toml
  macOS:   ~/Library/Application Support/tessitura/taxonomy.toml
  Windows: %APPDATA%\\tessitura\\taxonomy.toml

Examples:
  tessitura rules init                    # Create default rules file
  tessitura rules path                    # Show rules file location
  tessitura rules edit                    # Open rules in $EDITOR
  tessitura rules validate                # Check rules syntax

The rules file defines how raw metadata from enrichment sources
(MusicBrainz, Wikidata, Last.fm, Discogs) is mapped to controlled
vocabulary terms for genre, period, and instrumentation.")]
    Rules {
        #[command(subcommand)]
        action: RulesAction,
    },
    /// Manage configuration
    #[command(long_about = "View and modify tessitura configuration settings.

Config file location:
  Linux:   ~/.config/tessitura/config.toml
  macOS:   ~/Library/Application Support/tessitura/config.toml
  Windows: %APPDATA%\\tessitura\\config.toml

Examples:
  tessitura config                        # Show current config
  tessitura config get                    # Show config file contents
  tessitura config get acoustid_api_key   # Get top-level value
  tessitura config get logging.level      # Get nested value (dotted notation)
  tessitura config set acoustid_api_key \"your-key\"  # Set top-level value
  tessitura config set logging.level debug            # Set nested value
  tessitura config set logging.coloured false         # Auto-detects boolean
  tessitura config path                   # Show config file location
  tessitura config example                # Show example config
  tessitura config init                   # Create default config file

Dotted notation: Use dots to access nested config values (e.g., logging.level)")]
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Debug, clap::Subcommand)]
enum VocabAction {
    /// Load vocabulary snapshots into the database
    Load {
        /// Path to LCGFT snapshot file
        #[arg(long)]
        lcgft: Option<PathBuf>,
        /// Path to LCMPT snapshot file
        #[arg(long)]
        lcmpt: Option<PathBuf>,
    },
    /// Show vocabulary statistics
    Stats,
}

#[derive(Debug, clap::Subcommand)]
enum RulesAction {
    /// Initialize rules file with defaults
    Init,
    /// Show rules file path
    Path,
    /// Open rules file in $EDITOR
    Edit,
    /// Validate rules file syntax
    Validate,
}

#[derive(Debug, clap::Subcommand)]
enum ConfigAction {
    /// Get configuration value(s)
    Get {
        /// Specific key to get (acoustid_api_key, database_path)
        key: Option<String>,
    },
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show config file path
    Path,
    /// Show example configuration
    Example,
    /// Initialize config file with defaults
    Init,
}

// Removed: now using Config::load() which has default_db_path internally

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration from file and environment variables
    let config = if let Some(db_path) = cli.db {
        // CLI flag takes highest priority
        Config::load_with_db_path(db_path)?
    } else {
        Config::load()?
    };

    // Initialize logging with config
    if let Err(e) = twyg::setup(config.logging.clone()) {
        eprintln!("Warning: Failed to initialize logging: {}", e);
        eprintln!("Continuing with default stderr output");
    }

    // Ensure database directory exists
    if let Some(parent) = config.database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match cli.command {
        Commands::Process { path, resume } => {
            let db_path = config.database_path.clone();
            commands::run_process(path, db_path, &config, resume).await?;
        }
        Commands::Scan { path } => {
            commands::run_scan(path, config.database_path).await?;
        }
        Commands::Identify => {
            commands::run_identify(config.database_path, config.acoustid_api_key).await?;
        }
        Commands::Fingerprint { force } => {
            commands::run_fingerprint(config.database_path, force).await?;
        }
        Commands::Enrich { pending_only } => {
            let db_path = config.database_path.clone();
            commands::enrich::run_enrich(&config, db_path, pending_only).await?;
        }
        Commands::Harmonize => {
            commands::harmonize::run_harmonize(config.database_path, config.rules_path)?;
        }
        Commands::Review => {
            commands::review::run_review(config.database_path)?;
        }
        Commands::Status { filter } => {
            commands::show_status(config.database_path, filter)?;
        }
        Commands::Vocab { action } => match action {
            VocabAction::Load { lcgft, lcmpt } => {
                commands::vocab::load_vocab(config.database_path, lcgft, lcmpt)?;
            }
            VocabAction::Stats => {
                commands::vocab::vocab_stats(config.database_path)?;
            }
        },
        Commands::Rules { action } => match action {
            RulesAction::Init => {
                commands::rules::init_rules()?;
            }
            RulesAction::Path => {
                commands::rules::show_path()?;
            }
            RulesAction::Edit => {
                commands::rules::edit_rules()?;
            }
            RulesAction::Validate => {
                commands::rules::validate_rules()?;
            }
        },
        Commands::Config { action } => {
            match action {
                None => {
                    // No subcommand, show current config
                    commands::config::show_config()?;
                }
                Some(ConfigAction::Get { key }) => {
                    commands::config::get_config(key)?;
                }
                Some(ConfigAction::Set { key, value }) => {
                    commands::config::set_config(key, value)?;
                }
                Some(ConfigAction::Path) => {
                    commands::config::show_path()?;
                }
                Some(ConfigAction::Example) => {
                    commands::config::show_example()?;
                }
                Some(ConfigAction::Init) => {
                    commands::config::init_config()?;
                }
            }
        }
    }

    Ok(())
}
