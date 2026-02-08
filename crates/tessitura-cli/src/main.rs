use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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
    Scan {
        /// Path to the music directory
        path: PathBuf,
    },
    /// Identify recordings via AcoustID/MusicBrainz
    Identify {
        /// Path to the music directory (for context)
        #[arg(long)]
        music_dir: Option<PathBuf>,
    },
    /// Show pipeline status
    Status {
        /// Optional filter (album name, artist, etc.)
        filter: Option<String>,
    },
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("tessitura.db")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let db_path = cli.db.unwrap_or_else(default_db_path);

    // Ensure database directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match cli.command {
        Commands::Scan { path } => {
            commands::run_scan(path, db_path).await?;
        }
        Commands::Identify { music_dir } => {
            // Get AcoustID API key from environment
            let acoustid_api_key = std::env::var("ACOUSTID_API_KEY").ok();

            // Use current directory if no music_dir specified
            let music_dir = music_dir.unwrap_or_else(|| PathBuf::from("."));

            commands::run_identify(music_dir, db_path, acoustid_api_key).await?;
        }
        Commands::Status { filter } => {
            commands::show_status(db_path, filter)?;
        }
    }

    Ok(())
}
