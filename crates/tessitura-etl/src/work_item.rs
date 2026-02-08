use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use treadle::WorkItem;

/// A music file being processed through the pipeline.
///
/// This is the treadle `WorkItem` that flows through the scan → identify
/// → enrich → harmonize → review → index → export stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicFile {
    /// Unique ID for this work item (matches the Item ID in the database).
    id: String,
    /// Path to the audio file.
    pub path: PathBuf,
}

impl MusicFile {
    #[must_use]
    pub fn new(id: impl Into<String>, path: PathBuf) -> Self {
        Self {
            id: id.into(),
            path,
        }
    }
}

impl WorkItem for MusicFile {
    fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for MusicFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_music_file_creation() {
        let file = MusicFile::new("test-id", PathBuf::from("/music/test.flac"));
        assert_eq!(file.id(), "test-id");
        assert_eq!(file.path, PathBuf::from("/music/test.flac"));
    }

    #[test]
    fn test_music_file_display() {
        let file = MusicFile::new("test-id", PathBuf::from("/music/test.flac"));
        let display = format!("{file}");
        assert!(display.contains("test.flac"));
    }
}
