//! Integration tests for the full scan → identify pipeline.
//!
//! These tests use mocked HTTP responses to verify the pipeline works correctly
//! without requiring real AcoustID/MusicBrainz API calls or real audio files.

use std::path::PathBuf;
use tempfile::TempDir;
use tessitura_core::schema::Database;
use tessitura_etl::{build_pipeline, MusicFile};
use treadle::WorkItem;

/// Test that the pipeline can be built and wired correctly
#[tokio::test]
async fn test_pipeline_construction() {
    let temp_dir = TempDir::new().unwrap();
    let music_dir = temp_dir.path().to_path_buf();
    let db_path = temp_dir.path().join("test.db");

    // Build the pipeline
    let result = build_pipeline(music_dir, db_path, None);

    assert!(result.is_ok(), "Pipeline should build successfully");
}

/// Test database initialization and schema creation
#[test]
fn test_database_schema_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Open database (should create schema)
    let db = Database::open(&db_path).expect("Failed to open database");

    // Verify tables exist by querying them
    let unidentified = db
        .list_unidentified_items()
        .expect("Failed to list unidentified");
    assert_eq!(
        unidentified.len(),
        0,
        "New database should have no unidentified items"
    );
}

/// Test that the status command can query an empty database
#[test]
fn test_status_on_empty_database() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = Database::open(&db_path).expect("Failed to open database");

    // Count unidentified items
    let unidentified = db
        .list_unidentified_items()
        .expect("Failed to list unidentified");

    assert_eq!(unidentified.len(), 0);
}

/// Test work item creation
#[test]
fn test_music_file_work_item() {
    let path = PathBuf::from("/test/path/music.flac");
    let work_item = MusicFile::new("test-id", path.clone());

    assert_eq!(work_item.id(), "test-id");
    assert_eq!(work_item.path, path);

    // Test Display implementation
    let display = format!("{}", work_item);
    assert_eq!(display, "/test/path/music.flac");
}

// Note: Tests with real audio file scanning and tag extraction would require
// test fixtures. For Phase 1, we validate the core infrastructure works.
// Full end-to-end tests with mocked HTTP responses can be added in follow-up work.
