use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::model::ids::{ExpressionId, ItemId, ManifestationId};

/// The format of an audio file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioFormat {
    Flac,
    Mp3,
    Ogg,
    Wav,
    Aac,
    Other,
}

impl AudioFormat {
    /// Detect format from a file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "flac" => Self::Flac,
            "mp3" => Self::Mp3,
            "ogg" | "oga" => Self::Ogg,
            "wav" => Self::Wav,
            "aac" | "m4a" => Self::Aac,
            _ => Self::Other,
        }
    }
}

/// A specific audio file on disk.
///
/// Corresponds to the FRBR "Item" level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,

    /// Links to the recording this file contains (set during Identify stage).
    pub expression_id: Option<ExpressionId>,

    /// Links to the release this file belongs to (set during Identify stage).
    pub manifestation_id: Option<ManifestationId>,

    /// Absolute path to the audio file.
    pub file_path: PathBuf,

    /// Audio format.
    pub format: AudioFormat,

    /// File size in bytes.
    pub file_size: u64,

    /// File modification time (for change detection).
    pub file_mtime: DateTime<Utc>,

    /// SHA-256 hash of the file content (for change detection).
    pub file_hash: Option<String>,

    /// `AcoustID` fingerprint (computed during Scan stage).
    pub fingerprint: Option<String>,

    /// `AcoustID` score (confidence of the fingerprint match, 0.0-1.0).
    pub fingerprint_score: Option<f64>,

    // --- Embedded tag metadata (extracted during Scan stage) ---
    /// Track title as read from embedded tags.
    pub tag_title: Option<String>,

    /// Artist as read from embedded tags.
    pub tag_artist: Option<String>,

    /// Album as read from embedded tags.
    pub tag_album: Option<String>,

    /// Album artist as read from embedded tags.
    pub tag_album_artist: Option<String>,

    /// Track number as read from embedded tags.
    pub tag_track_number: Option<u32>,

    /// Disc number as read from embedded tags.
    pub tag_disc_number: Option<u32>,

    /// Year as read from embedded tags.
    pub tag_year: Option<i32>,

    /// Genre as read from embedded tags.
    pub tag_genre: Option<String>,

    /// Duration in seconds as read from file properties.
    pub duration_secs: Option<f64>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Item {
    #[must_use]
    pub fn new(
        file_path: PathBuf,
        format: AudioFormat,
        file_size: u64,
        file_mtime: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: ItemId::new(),
            expression_id: None,
            manifestation_id: None,
            file_path,
            format,
            file_size,
            file_mtime,
            file_hash: None,
            fingerprint: None,
            fingerprint_score: None,
            tag_title: None,
            tag_artist: None,
            tag_album: None,
            tag_album_artist: None,
            tag_track_number: None,
            tag_disc_number: None,
            tag_year: None,
            tag_genre: None,
            duration_secs: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Whether this item has been identified (linked to an Expression).
    #[must_use]
    pub const fn is_identified(&self) -> bool {
        self.expression_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_audio_format_from_extension() {
        assert_eq!(AudioFormat::from_extension("flac"), AudioFormat::Flac);
        assert_eq!(AudioFormat::from_extension("FLAC"), AudioFormat::Flac);
        assert_eq!(AudioFormat::from_extension("mp3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("ogg"), AudioFormat::Ogg);
        assert_eq!(AudioFormat::from_extension("wav"), AudioFormat::Wav);
        assert_eq!(AudioFormat::from_extension("m4a"), AudioFormat::Aac);
        assert_eq!(AudioFormat::from_extension("xyz"), AudioFormat::Other);
    }

    #[test]
    fn test_item_new() {
        let path = Path::new("/music/test.flac").to_path_buf();
        let now = Utc::now();
        let item = Item::new(path.clone(), AudioFormat::Flac, 1024, now);

        assert_eq!(item.file_path, path);
        assert_eq!(item.format, AudioFormat::Flac);
        assert_eq!(item.file_size, 1024);
        assert!(!item.is_identified());
    }

    #[test]
    fn test_item_is_identified() {
        let item = Item::new(
            PathBuf::from("/music/test.flac"),
            AudioFormat::Flac,
            1024,
            Utc::now(),
        );
        assert!(!item.is_identified());

        let mut item_with_expr = item;
        item_with_expr.expression_id = Some(ExpressionId::new());
        assert!(item_with_expr.is_identified());
    }
}
