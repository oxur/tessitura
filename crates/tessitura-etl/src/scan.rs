use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use std::path::{Path, PathBuf};
use tessitura_core::model::{AudioFormat, Item};
use tessitura_core::schema::Database;
use treadle::{Stage, StageContext, StageOutcome};
use walkdir::WalkDir;

/// Tags extracted from an audio file.
#[derive(Debug, Default)]
struct TagData {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    year: Option<i32>,
    genre: Option<String>,
    duration_secs: Option<f64>,
}

/// The Scan stage: walk directory, extract tags, create Item records.
#[derive(Debug)]
pub struct ScanStage {
    music_dir: PathBuf,
    db_path: PathBuf,
}

impl ScanStage {
    #[must_use]
    pub fn new(music_dir: PathBuf, db_path: PathBuf) -> Self {
        Self { music_dir, db_path }
    }

    fn is_audio_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            matches!(
                ext.to_string_lossy().to_lowercase().as_ref(),
                "flac" | "mp3" | "ogg" | "oga" | "wav" | "m4a" | "aac"
            )
        } else {
            false
        }
    }

    fn extract_tags(path: &Path) -> Result<TagData, Box<dyn std::error::Error>> {
        let tagged_file = lofty::read_from_path(path)?;

        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());

        let properties = tagged_file.properties();

        let mut tag_data = TagData {
            duration_secs: Some(properties.duration().as_secs_f64()),
            ..Default::default()
        };

        if let Some(tag) = tag {
            tag_data.title = tag.title().map(|s| s.to_string());
            tag_data.artist = tag.artist().map(|s| s.to_string());
            tag_data.album = tag.album().map(|s| s.to_string());
            tag_data.track_number = tag.track();
            tag_data.disc_number = tag.disk();
            tag_data.year = tag.year().map(|y| y as i32);
            tag_data.genre = tag.genre().map(|s| s.to_string());

            // Try to get album artist from specific tag items
            // This is format-specific, but we'll try the common ones
            tag_data.album_artist = tag
                .get_string(&lofty::prelude::ItemKey::AlbumArtist)
                .map(|s| s.to_string());
        }

        Ok(tag_data)
    }

    fn scan_directory(&self, db: &Database) -> Result<usize, Box<dyn std::error::Error>> {
        let mut count = 0;

        for entry in WalkDir::new(&self.music_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() || !Self::is_audio_file(path) {
                continue;
            }

            log::debug!("Scanning: {}", path.display());

            // Extract format and metadata
            let format = path
                .extension()
                .map(|ext| AudioFormat::from_extension(&ext.to_string_lossy()))
                .unwrap_or(AudioFormat::Other);

            let metadata = std::fs::metadata(path)?;
            let file_size = metadata.len();
            let file_mtime = metadata.modified()?.into();

            // Extract tags
            let tags = match Self::extract_tags(path) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Failed to extract tags from {}: {}", path.display(), e);
                    TagData::default()
                }
            };

            // Create Item record
            let mut item = Item::new(path.to_path_buf(), format, file_size, file_mtime);
            item.tag_title = tags.title;
            item.tag_artist = tags.artist;
            item.tag_album = tags.album;
            item.tag_album_artist = tags.album_artist;
            item.tag_track_number = tags.track_number;
            item.tag_disc_number = tags.disc_number;
            item.tag_year = tags.year;
            item.tag_genre = tags.genre;
            item.duration_secs = tags.duration_secs;

            // TODO: Compute fingerprint (stub for now)
            item.fingerprint = None;

            // Insert or update in database
            db.insert_item(&item)?;
            count += 1;
        }

        Ok(count)
    }
}

#[async_trait::async_trait]
impl Stage for ScanStage {
    fn name(&self) -> &str {
        "scan"
    }

    async fn execute(
        &self,
        _item: &dyn treadle::WorkItem,
        _context: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        log::info!("Starting scan of {}", self.music_dir.display());

        let db = Database::open(&self.db_path).map_err(|e| {
            treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
        })?;

        match self.scan_directory(&db) {
            Ok(count) => {
                log::info!("Scan complete: {} files processed", count);
                Ok(StageOutcome::Complete)
            }
            Err(e) => Err(treadle::TreadleError::StageExecution(format!(
                "Scan failed: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_audio_file() {
        assert!(ScanStage::is_audio_file(Path::new("/music/test.flac")));
        assert!(ScanStage::is_audio_file(Path::new("/music/test.mp3")));
        assert!(ScanStage::is_audio_file(Path::new("/music/test.ogg")));
        assert!(!ScanStage::is_audio_file(Path::new("/music/test.txt")));
        assert!(!ScanStage::is_audio_file(Path::new("/music/test")));
    }

    #[tokio::test]
    async fn test_scan_stage_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = temp_dir.path().to_path_buf();
        let db_path = temp_dir.path().join("test.db");

        let stage = ScanStage::new(music_dir.clone(), db_path.clone());
        let db = Database::open(&db_path).unwrap();

        let result = stage.scan_directory(&db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_scan_stage_with_non_audio_files() {
        let temp_dir = TempDir::new().unwrap();
        let music_dir = temp_dir.path().to_path_buf();
        let db_path = temp_dir.path().join("test.db");

        // Create some non-audio files
        fs::write(music_dir.join("test.txt"), "not audio").unwrap();
        fs::write(music_dir.join("readme.md"), "# README").unwrap();

        let stage = ScanStage::new(music_dir.clone(), db_path.clone());
        let db = Database::open(&db_path).unwrap();

        let result = stage.scan_directory(&db);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // No audio files found
    }
}
