use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;
use crate::model::{Artist, Expression, Item, Manifestation, Work};
use crate::provenance::Assertion;

use super::migrations::MIGRATIONS;

/// A database connection with CRUD methods for FRBR entities.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) a database at the given path and apply migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.apply_migrations()?;
        Ok(db)
    }

    /// Open an in-memory database (for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.apply_migrations()?;
        Ok(db)
    }

    /// Get a reference to the underlying connection (for advanced queries).
    #[must_use]
    pub const fn conn(&self) -> &Connection {
        &self.conn
    }

    fn apply_migrations(&self) -> Result<()> {
        // Create migrations table if it doesn't exist
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Get applied migrations
        let mut stmt = self
            .conn
            .prepare("SELECT version FROM schema_migrations ORDER BY version")?;
        let applied: Vec<u32> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Apply pending migrations
        for migration in MIGRATIONS {
            if !applied.contains(&migration.version) {
                log::info!(
                    "Applying migration {} ({})",
                    migration.version,
                    migration.name
                );
                self.conn.execute_batch(migration.sql)?;
                self.conn.execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    rusqlite::params![migration.version, migration.name],
                )?;
            }
        }

        Ok(())
    }
}

// Item CRUD
impl Database {
    /// Insert a new item.
    pub fn insert_item(&self, item: &Item) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (
                id, expression_id, manifestation_id, file_path, format,
                file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                tag_title, tag_artist, tag_album, tag_album_artist,
                tag_track_number, tag_disc_number, tag_year, tag_genre,
                duration_secs, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            rusqlite::params![
                item.id.to_string(),
                item.expression_id.map(|id| id.to_string()),
                item.manifestation_id.map(|id| id.to_string()),
                item.file_path.to_string_lossy().as_ref(),
                format!("{:?}", item.format),
                i64::try_from(item.file_size).unwrap_or(0),
                item.file_mtime.to_rfc3339(),
                item.file_hash,
                item.fingerprint,
                item.fingerprint_score,
                item.tag_title,
                item.tag_artist,
                item.tag_album,
                item.tag_album_artist,
                item.tag_track_number.map(i64::from),
                item.tag_disc_number.map(i64::from),
                item.tag_year.map(i64::from),
                item.tag_genre,
                item.duration_secs,
                item.created_at.to_rfc3339(),
                item.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Update an existing item.
    pub fn update_item(&self, item: &Item) -> Result<()> {
        self.conn.execute(
            "UPDATE items SET
                expression_id = ?2, manifestation_id = ?3, file_path = ?4,
                format = ?5, file_size = ?6, file_mtime = ?7, file_hash = ?8,
                fingerprint = ?9, fingerprint_score = ?10,
                tag_title = ?11, tag_artist = ?12, tag_album = ?13,
                tag_album_artist = ?14, tag_track_number = ?15,
                tag_disc_number = ?16, tag_year = ?17, tag_genre = ?18,
                duration_secs = ?19, updated_at = ?20
             WHERE id = ?1",
            rusqlite::params![
                item.id.to_string(),
                item.expression_id.map(|id| id.to_string()),
                item.manifestation_id.map(|id| id.to_string()),
                item.file_path.to_string_lossy().as_ref(),
                format!("{:?}", item.format),
                i64::try_from(item.file_size).unwrap_or(0),
                item.file_mtime.to_rfc3339(),
                item.file_hash,
                item.fingerprint,
                item.fingerprint_score,
                item.tag_title,
                item.tag_artist,
                item.tag_album,
                item.tag_album_artist,
                item.tag_track_number.map(i64::from),
                item.tag_disc_number.map(i64::from),
                item.tag_year.map(i64::from),
                item.tag_genre,
                item.duration_secs,
                item.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// List all unidentified items (no expression_id).
    pub fn list_unidentified_items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             WHERE expression_id IS NULL
             ORDER BY file_path",
        )?;

        let items = stmt
            .query_map([], |row| self.row_to_item(row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    fn row_to_item(&self, row: &rusqlite::Row) -> rusqlite::Result<Item> {
        use crate::model::{AudioFormat, ExpressionId, ItemId, ManifestationId};
        use chrono::DateTime;
        use std::path::PathBuf;
        use uuid::Uuid;

        let id = ItemId::from_uuid(Uuid::parse_str(&row.get::<_, String>(0)?).unwrap());
        let expression_id: Option<String> = row.get(1)?;
        let manifestation_id: Option<String> = row.get(2)?;
        let file_path: String = row.get(3)?;
        let format_str: String = row.get(4)?;
        let file_size: i64 = row.get(5)?;
        let file_mtime_str: String = row.get(6)?;
        let created_at_str: String = row.get(19)?;
        let updated_at_str: String = row.get(20)?;

        Ok(Item {
            id,
            expression_id: expression_id
                .map(|s| ExpressionId::from_uuid(Uuid::parse_str(&s).unwrap())),
            manifestation_id: manifestation_id
                .map(|s| ManifestationId::from_uuid(Uuid::parse_str(&s).unwrap())),
            file_path: PathBuf::from(file_path),
            format: match format_str.as_str() {
                "Flac" => AudioFormat::Flac,
                "Mp3" => AudioFormat::Mp3,
                "Ogg" => AudioFormat::Ogg,
                "Wav" => AudioFormat::Wav,
                "Aac" => AudioFormat::Aac,
                _ => AudioFormat::Other,
            },
            file_size: file_size as u64,
            file_mtime: DateTime::parse_from_rfc3339(&file_mtime_str)
                .unwrap()
                .into(),
            file_hash: row.get(7)?,
            fingerprint: row.get(8)?,
            fingerprint_score: row.get(9)?,
            tag_title: row.get(10)?,
            tag_artist: row.get(11)?,
            tag_album: row.get(12)?,
            tag_album_artist: row.get(13)?,
            tag_track_number: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
            tag_disc_number: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
            tag_year: row.get::<_, Option<i64>>(16)?.map(|v| v as i32),
            tag_genre: row.get(17)?,
            duration_secs: row.get(18)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .into(),
        })
    }
}

// Assertion CRUD
impl Database {
    /// Insert a new assertion.
    pub fn insert_assertion(&self, assertion: &Assertion) -> Result<()> {
        self.conn.execute(
            "INSERT INTO assertions (entity_id, field, value, source, confidence, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                assertion.entity_id,
                assertion.field,
                serde_json::to_string(&assertion.value)?,
                format!("{:?}", assertion.source),
                assertion.confidence,
                assertion.fetched_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get all assertions for an entity.
    pub fn get_assertions_for_entity(&self, entity_id: &str) -> Result<Vec<Assertion>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_id, field, value, source, confidence, fetched_at
             FROM assertions
             WHERE entity_id = ?1
             ORDER BY fetched_at DESC",
        )?;

        let assertions = stmt
            .query_map([entity_id], |row| self.row_to_assertion(row))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(assertions)
    }

    fn row_to_assertion(&self, row: &rusqlite::Row) -> rusqlite::Result<Assertion> {
        use crate::provenance::Source;
        use chrono::DateTime;

        let entity_id: String = row.get(0)?;
        let field: String = row.get(1)?;
        let value_str: String = row.get(2)?;
        let source_str: String = row.get(3)?;
        let confidence: Option<f64> = row.get(4)?;
        let fetched_at_str: String = row.get(5)?;

        let value = serde_json::from_str(&value_str).unwrap_or(serde_json::Value::Null);

        let source = match source_str.as_str() {
            "EmbeddedTag" => Source::EmbeddedTag,
            "AcoustId" => Source::AcoustId,
            "MusicBrainz" => Source::MusicBrainz,
            "Wikidata" => Source::Wikidata,
            "LastFm" => Source::LastFm,
            "Lcgft" => Source::Lcgft,
            "Lcmpt" => Source::Lcmpt,
            "Discogs" => Source::Discogs,
            "User" => Source::User,
            _ => Source::User,
        };

        Ok(Assertion {
            entity_id,
            field,
            value,
            source,
            confidence,
            fetched_at: DateTime::parse_from_rfc3339(&fetched_at_str)
                .unwrap()
                .into(),
        })
    }
}

// Stub implementations for Work, Expression, Manifestation, Artist
// These will be expanded as needed in later milestones
impl Database {
    /// Insert a work (stub - will be expanded).
    pub fn insert_work(&self, _work: &Work) -> Result<()> {
        todo!("Implement in milestone 1.6")
    }

    /// Insert an expression (stub - will be expanded).
    pub fn insert_expression(&self, _expr: &Expression) -> Result<()> {
        todo!("Implement in milestone 1.6")
    }

    /// Insert a manifestation (stub - will be expanded).
    pub fn insert_manifestation(&self, _man: &Manifestation) -> Result<()> {
        todo!("Implement in milestone 1.6")
    }

    /// Insert an artist (stub - will be expanded).
    pub fn insert_artist(&self, _artist: &Artist) -> Result<()> {
        todo!("Implement in milestone 1.6")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioFormat, Item};
    use crate::provenance::{Assertion, Source};
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn test_database_open_in_memory() {
        let db = Database::open_in_memory().unwrap();
        // Verify migrations table exists
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1); // One migration applied
    }

    #[test]
    fn test_item_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();

        let mut item = Item::new(
            PathBuf::from("/music/test.flac"),
            AudioFormat::Flac,
            1024,
            now,
        );
        item.tag_title = Some("Test Track".to_string());
        item.tag_artist = Some("Test Artist".to_string());

        db.insert_item(&item).unwrap();

        let items = db.list_unidentified_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_path, item.file_path);
        assert_eq!(items[0].tag_title, Some("Test Track".to_string()));
    }

    #[test]
    fn test_assertion_round_trip() {
        let db = Database::open_in_memory().unwrap();

        let assertion = Assertion::new(
            "entity-123",
            "genre",
            serde_json::json!("Classical"),
            Source::MusicBrainz,
        )
        .with_confidence(0.95);

        db.insert_assertion(&assertion).unwrap();

        let assertions = db.get_assertions_for_entity("entity-123").unwrap();
        assert_eq!(assertions.len(), 1);
        assert_eq!(assertions[0].field, "genre");
        assert_eq!(assertions[0].source, Source::MusicBrainz);
        assert_eq!(assertions[0].confidence, Some(0.95));
    }
}
