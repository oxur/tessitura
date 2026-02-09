use anyhow::{Context, Result};
use confyg::{env, Confygery};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for tessitura.
///
/// Configuration is loaded from multiple sources with the following priority:
/// 1. CLI arguments (highest priority)
/// 2. Environment variables (TESS_* prefix)
/// 3. Config file (~/.config/tessitura/config.toml)
/// 4. Built-in defaults (lowest priority)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AcoustID API key (required for fingerprint matching).
    ///
    /// Can be set via:
    /// - ENV: TESS_ACOUSTID_API_KEY
    /// - Config: acoustid_api_key = "..."
    pub acoustid_api_key: Option<String>,

    /// Discogs personal access token for release-level enrichment.
    ///
    /// Optional. Authenticated requests benefit from a higher rate limit
    /// (240 req/min vs 60 req/min unauthenticated).
    ///
    /// Can be set via:
    /// - ENV: TESS_DISCOGS_TOKEN
    /// - Config: discogs_token = "..."
    ///
    /// Generate a token at: <https://www.discogs.com/settings/developers>
    #[serde(default)]
    pub discogs_token: Option<String>,

    /// Last.fm API key for folksonomy tag enrichment.
    ///
    /// Can be set via:
    /// - ENV: TESS_LASTFM_API_KEY
    /// - Config: lastfm_api_key = "..."
    ///
    /// Register for a free API key at: <https://www.last.fm/api/account/create>
    #[serde(default)]
    pub lastfm_api_key: Option<String>,

    /// Path to the SQLite database.
    ///
    /// Can be set via:
    /// - CLI: --db /path/to/db
    /// - ENV: TESS_DATABASE_PATH
    /// - Config: database_path = "/path/to/db"
    /// - Default: ~/.local/share/tessitura/tessitura.db
    #[serde(default = "default_db_path")]
    pub database_path: PathBuf,

    /// Path to the mapping rules TOML file for harmonization.
    ///
    /// Can be set via:
    /// - ENV: TESS_RULES_PATH
    /// - Config: rules_path = "/path/to/taxonomy.toml"
    /// - Default: {config_dir}/tessitura/taxonomy.toml
    #[serde(default = "default_rules_path")]
    pub rules_path: PathBuf,

    /// Logging configuration.
    ///
    /// Can be set via:
    /// - ENV: TESS_LOGGING_* (e.g., TESS_LOGGING_LEVEL=debug)
    /// - Config: [logging] section
    /// - Default: Info level, colored output to stdout
    #[serde(default = "default_logging")]
    pub logging: twyg::Opts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            acoustid_api_key: None,
            discogs_token: None,
            lastfm_api_key: None,
            database_path: default_db_path(),
            rules_path: default_rules_path(),
            logging: default_logging(),
        }
    }
}

impl Config {
    /// Load configuration from file and environment variables.
    ///
    /// Searches for config file at: ~/.config/tessitura/config.toml
    /// Reads environment variables with TESS_ prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be parsed.
    pub fn load() -> Result<Self> {
        let config_path = config_file_path();

        // Create Confygery builder
        let mut builder = Confygery::new().context("Failed to create config builder")?;

        // If config file exists, load it
        if config_path.exists() {
            let path_str = config_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Config path contains invalid UTF-8"))?;
            builder
                .add_file(path_str)
                .context("Failed to load config file")?;
        }

        // Set up environment variable scanning with TESS_ prefix
        let env_opts = env::Options::with_top_level("tess");
        builder
            .add_env(env_opts)
            .context("Failed to load environment variables")?;

        // Build and deserialize into Config
        let config: Self = builder.build().context("Failed to build configuration")?;

        Ok(config)
    }

    /// Load configuration with custom database path.
    ///
    /// This is used when the --db CLI flag is provided.
    pub fn load_with_db_path(db_path: PathBuf) -> Result<Self> {
        let mut config = Self::load()?;
        config.database_path = db_path;
        Ok(config)
    }
}

/// Get the default database path.
///
/// Returns: ~/.local/share/tessitura/tessitura.db (or platform equivalent)
fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("tessitura.db")
}

/// Get the default rules path.
///
/// Returns: {config_dir}/tessitura/taxonomy.toml
fn default_rules_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("taxonomy.toml")
}

/// Get the default logging configuration.
///
/// Returns twyg::Opts configured with:
/// - Info level logging
/// - Colored output
/// - Output to stdout
/// - No caller reporting
/// - Padded level names
fn default_logging() -> twyg::Opts {
    twyg::OptsBuilder::new()
        .coloured(true)
        .output(twyg::Output::Stdout)
        .level(twyg::LogLevel::Info)
        .report_caller(false)
        .pad_level(true)
        .build()
        .unwrap_or_else(|e| {
            eprintln!("Warning: Failed to build logging config: {e}, using fallback");
            twyg::Opts::default()
        })
}

/// Get the config file path.
///
/// Returns:
/// - Linux: ~/.config/tessitura/config.toml
/// - macOS: ~/Library/Application Support/tessitura/config.toml
/// - Windows: %APPDATA%\tessitura\config.toml
pub fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("config.toml")
}

/// Get the example config file content.
#[allow(clippy::too_many_lines)]
pub fn example_config() -> &'static str {
    r###"# Tessitura Configuration File
#
# Configuration is loaded from multiple sources with the following priority:
# 1. CLI arguments (highest priority)
# 2. Environment variables (TESS_* prefix)
# 3. This config file
# 4. Built-in defaults (lowest priority)

# AcoustID API key for fingerprint matching
# Required for identifying recordings via audio fingerprinting
#
# Register for a free API key at: https://acoustid.org/new-application
#
# Can also be set via:
# - Environment: TESS_ACOUSTID_API_KEY=your-key-here
acoustid_api_key = "your-acoustid-api-key-here"

# Discogs personal access token for release-level enrichment
# Optional; unauthenticated requests are allowed but rate-limited more
# aggressively (60 req/min vs 240 req/min authenticated)
#
# Generate a token at: https://www.discogs.com/settings/developers
#
# Can also be set via:
# - Environment: TESS_DISCOGS_TOKEN=your-token-here
#discogs_token = "your-discogs-token-here"

# Last.fm API key for folksonomy tag enrichment
# Required for fetching community-driven genre/mood/style tags
#
# Register for a free API key at: https://www.last.fm/api/account/create
#
# Can also be set via:
# - Environment: TESS_LASTFM_API_KEY=your-key-here
#lastfm_api_key = "your-lastfm-api-key-here"

# Path to the SQLite database
#
# Stores all catalog data including Works, Expressions, Manifestations, and Items
#
# Can also be set via:
# - CLI: tessitura --db /custom/path.db scan /music
# - Environment: TESS_DATABASE_PATH=/custom/path.db
#
# Default: Platform-specific data directory
#database_path = "/path/to/custom/tessitura.db"

# Path to the mapping rules TOML file for harmonization
#
# The rules file controls how enrichment data from multiple sources
# (MusicBrainz, Wikidata, Last.fm, Discogs) is normalized into canonical
# genre, form, period, and instrumentation values.
#
# Can also be set via:
# - Environment: TESS_RULES_PATH=/path/to/taxonomy.toml
#
# Default: {config_dir}/tessitura/taxonomy.toml
#rules_path = "/path/to/taxonomy.toml"

# Logging configuration
#
# All options can also be set via environment variables with TESS_LOGGING_* prefix
# Examples:
#   TESS_LOGGING_LEVEL=debug
#   TESS_LOGGING_COLOURED=false
#   TESS_LOGGING_OUTPUT=stderr
[logging]
# Enable colored output (true/false)
# Default: true
coloured = true

# Output destination: "stdout" or "stderr"
# Default: "stdout"
output = "stdout"

# Log level: "trace", "debug", "info", "warn", "error"
# Default: "info"
level = "info"

# Report caller location (file and line number)
# Default: false
report_caller = false

# Timestamp format (strftime format string)
# Default: "%Y-%m-%d %H:%M:%S"
#timestamp_format = "%Y-%m-%d %H:%M:%S"

# Pad level names to equal width
# Default: true
pad_level = true

# Padding amount (number of spaces)
# Default: 1
#pad_amount = 1

# Padding side: "left" or "right"
# Default: "right"
#pad_side = "right"

# Message separator (between metadata and message)
# Default: " "
#msg_separator = " "

# Arrow character (for structured formatting)
# Default: "→"
#arrow_char = "→"

# Color configuration
# All colors support: black, red, green, yellow, blue, purple, cyan, white
# Or hex colors: "#ff0000"
[logging.colors]
# Timestamp color
#timestamp = "cyan"

# Log level colors
#level_trace = "purple"
#level_debug = "blue"
#level_info = "green"
#level_warn = "yellow"
#level_error = "red"

# Caller location color
#caller = "cyan"

# Message color
#message = "white"

# Key-value pair colors
#key = "cyan"
#value = "white"
#separator = "white"
"###
}

/// Create default config file if it doesn't exist.
///
/// Returns true if a new file was created, false if it already existed.
pub fn ensure_config_file() -> Result<bool> {
    let config_path = config_file_path();

    if config_path.exists() {
        return Ok(false);
    }

    // Create parent directory
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    // Write default config
    std::fs::write(&config_path, example_config()).context("Failed to write config file")?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.database_path.as_os_str().is_empty());
        assert!(config.acoustid_api_key.is_none());
        assert!(config.discogs_token.is_none());
        assert!(config.lastfm_api_key.is_none());
        assert!(config.rules_path.ends_with("taxonomy.toml"));
        assert_eq!(config.logging.level(), twyg::LogLevel::Info);
        assert!(config.logging.coloured());
    }

    #[test]
    fn test_config_load() {
        // Should not fail even if config file doesn't exist
        let result = Config::load();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_with_custom_db_path() {
        let custom_path = PathBuf::from("/tmp/test.db");
        let config = Config::load_with_db_path(custom_path.clone());
        assert!(config.is_ok());
        assert_eq!(config.unwrap().database_path, custom_path);
    }
}
