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

    /// Path to the SQLite database.
    ///
    /// Can be set via:
    /// - CLI: --db /path/to/db
    /// - ENV: TESS_DATABASE_PATH
    /// - Config: database_path = "/path/to/db"
    /// - Default: ~/.local/share/tessitura/tessitura.db
    #[serde(default = "default_db_path")]
    pub database_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            acoustid_api_key: None,
            database_path: default_db_path(),
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
        let mut builder = Confygery::new()
            .context("Failed to create config builder")?;

        // If config file exists, load it
        if config_path.exists() {
            let path_str = config_path.to_str()
                .ok_or_else(|| anyhow::anyhow!("Config path contains invalid UTF-8"))?;
            builder.add_file(path_str)
                .context("Failed to load config file")?;
        }

        // Set up environment variable scanning with TESS_ prefix
        let env_opts = env::Options::with_top_level("tess");
        builder.add_env(env_opts)
            .context("Failed to load environment variables")?;

        // Build and deserialize into Config
        let config: Self = builder.build()
            .context("Failed to build configuration")?;

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

/// Get the config file path.
///
/// Returns: ~/.config/tessitura/config.toml (or platform equivalent)
fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.database_path.as_os_str().is_empty());
        assert!(config.acoustid_api_key.is_none());
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
