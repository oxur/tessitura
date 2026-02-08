use std::path::PathBuf;

/// Configuration for the ETL pipeline.
#[derive(Debug, Clone)]
pub struct Config {
    /// AcoustID API key (required for fingerprint matching).
    pub acoustid_api_key: Option<String>,
    /// Path to the SQLite database.
    pub database_path: PathBuf,
    /// Path to the music directory.
    pub music_dir: PathBuf,
}

impl Config {
    /// Load configuration from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            acoustid_api_key: std::env::var("ACOUSTID_API_KEY").ok(),
            database_path: default_db_path(),
            music_dir: PathBuf::new(), // Will be set from CLI args
        }
    }

    #[must_use]
    pub fn with_database_path(mut self, path: PathBuf) -> Self {
        self.database_path = path;
        self
    }

    #[must_use]
    pub fn with_music_dir(mut self, path: PathBuf) -> Self {
        self.music_dir = path;
        self
    }
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("tessitura.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        let config = Config::from_env();
        assert!(!config.database_path.as_os_str().is_empty());
    }

    #[test]
    fn test_config_builder() {
        let config = Config::from_env()
            .with_database_path(PathBuf::from("/tmp/test.db"))
            .with_music_dir(PathBuf::from("/music"));

        assert_eq!(config.database_path, PathBuf::from("/tmp/test.db"));
        assert_eq!(config.music_dir, PathBuf::from("/music"));
    }
}
