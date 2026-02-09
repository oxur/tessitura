use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;
use crate::model::{
    Artist, ArtistId, ArtistRole, Expression, ExpressionId, Item, ItemId, Manifestation,
    ManifestationId, Work, WorkId,
};
use crate::provenance::Assertion;
use crate::taxonomy::{LcgftTerm, LcmptTerm};

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

    /// Update only the identification fields of an item (avoids expensive clone).
    ///
    /// This is more efficient than `update_item` when only updating the
    /// identification-related fields after running the identify stage.
    pub fn update_item_identification(
        &self,
        item_id: &ItemId,
        expression_id: Option<ExpressionId>,
        manifestation_id: Option<ManifestationId>,
        fingerprint_score: Option<f64>,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        self.conn.execute(
            "UPDATE items SET
                expression_id = ?2, manifestation_id = ?3,
                fingerprint_score = ?4, updated_at = ?5
             WHERE id = ?1",
            rusqlite::params![
                item_id.to_string(),
                expression_id.map(|id| id.to_string()),
                manifestation_id.map(|id| id.to_string()),
                fingerprint_score,
                now.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// List all unidentified items (no `expression_id`).
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
            .query_map([], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    /// Get a single item by its ID.
    pub fn get_item_by_id(&self, id: &ItemId) -> Result<Option<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             WHERE id = ?1",
        )?;

        let mut items = stmt
            .query_map([id.to_string()], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items.pop())
    }

    /// Get a single item by its file path.
    pub fn get_item_by_path(&self, path: &std::path::Path) -> Result<Option<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             WHERE file_path = ?1",
        )?;

        let mut items = stmt
            .query_map([path.to_string_lossy().as_ref()], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items.pop())
    }

    /// List all identified items (those that have an `expression_id`).
    pub fn list_identified_items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             WHERE expression_id IS NOT NULL
             ORDER BY file_path",
        )?;

        let items = stmt
            .query_map([], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    /// List all items in the database.
    pub fn list_all_items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             ORDER BY file_path",
        )?;

        let items = stmt
            .query_map([], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    /// List items without fingerprints (for backfill command).
    pub fn list_items_without_fingerprints(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, expression_id, manifestation_id, file_path, format,
                    file_size, file_mtime, file_hash, fingerprint, fingerprint_score,
                    tag_title, tag_artist, tag_album, tag_album_artist,
                    tag_track_number, tag_disc_number, tag_year, tag_genre,
                    duration_secs, created_at, updated_at
             FROM items
             WHERE fingerprint IS NULL
             ORDER BY file_path",
        )?;

        let items = stmt
            .query_map([], Self::row_to_item)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(items)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<Item> {
        use crate::model::{AudioFormat, ExpressionId, ItemId, ManifestationId};
        use chrono::DateTime;
        use std::path::PathBuf;
        use uuid::Uuid;

        let id = ItemId::from_uuid(
            Uuid::parse_str(&row.get::<_, String>(0)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    0, rusqlite::types::Type::Text, Box::new(e)
                ))?
        );
        let expression_id: Option<String> = row.get(1)?;
        let manifestation_id: Option<String> = row.get(2)?;
        let file_path: String = row.get(3)?;
        let format_str: String = row.get(4)?;
        let file_size: i64 = row.get(5)?;
        let file_mtime_str: String = row.get(6)?;
        let created_at_str: String = row.get(19)?;
        let updated_at_str: String = row.get(20)?;

        // Helper to parse optional UUID with proper error handling
        let parse_optional_uuid = |s: Option<String>, col: usize| -> rusqlite::Result<Option<Uuid>> {
            s.map(|val|
                Uuid::parse_str(&val)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        col, rusqlite::types::Type::Text, Box::new(e)
                    ))
            ).transpose()
        };

        Ok(Item {
            id,
            expression_id: parse_optional_uuid(expression_id, 1)?
                .map(ExpressionId::from_uuid),
            manifestation_id: parse_optional_uuid(manifestation_id, 2)?
                .map(ManifestationId::from_uuid),
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
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    6, rusqlite::types::Type::Text, Box::new(e)
                ))?
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
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    19, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    20, rusqlite::types::Type::Text, Box::new(e)
                ))?
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
            .query_map([entity_id], Self::row_to_assertion)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(assertions)
    }

    fn row_to_assertion(row: &rusqlite::Row) -> rusqlite::Result<Assertion> {
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
            _ => Source::User,
        };

        Ok(Assertion {
            entity_id,
            field,
            value,
            source,
            confidence,
            fetched_at: DateTime::parse_from_rfc3339(&fetched_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    5, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
        })
    }
}

// Work CRUD
impl Database {
    /// Insert a new work.
    pub fn insert_work(&self, work: &Work) -> Result<()> {
        self.conn.execute(
            "INSERT INTO works (
                id, title, composer, musicbrainz_id, catalog_number,
                key, composed_year, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                work.id.to_string(),
                work.title,
                work.composer,
                work.musicbrainz_id,
                work.catalog_number,
                work.key,
                work.composed_year.map(i64::from),
                work.created_at.to_rfc3339(),
                work.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Insert or replace a work (upsert by primary key).
    pub fn upsert_work(&self, work: &Work) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO works (
                id, title, composer, musicbrainz_id, catalog_number,
                key, composed_year, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                work.id.to_string(),
                work.title,
                work.composer,
                work.musicbrainz_id,
                work.catalog_number,
                work.key,
                work.composed_year.map(i64::from),
                work.created_at.to_rfc3339(),
                work.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Look up a work by its `MusicBrainz` ID.
    pub fn get_work_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Work>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, composer, musicbrainz_id, catalog_number,
                    key, composed_year, created_at, updated_at
             FROM works
             WHERE musicbrainz_id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![mbid], Self::row_to_work)?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn row_to_work(row: &rusqlite::Row) -> rusqlite::Result<Work> {
        use chrono::DateTime;
        use uuid::Uuid;

        let id_str: String = row.get(0)?;
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        Ok(Work {
            id: WorkId::from_uuid(
                Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        0, rusqlite::types::Type::Text, Box::new(e)
                    ))?
            ),
            title: row.get(1)?,
            composer: row.get(2)?,
            musicbrainz_id: row.get(3)?,
            catalog_number: row.get(4)?,
            key: row.get(5)?,
            composed_year: row.get::<_, Option<i64>>(6)?.map(|v| v as i32),
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    7, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    8, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
        })
    }
}

// Expression CRUD
impl Database {
    /// Insert a new expression and its performer associations.
    pub fn insert_expression(&self, expr: &Expression) -> Result<()> {
        self.conn.execute(
            "INSERT INTO expressions (
                id, work_id, title, musicbrainz_id, conductor_id,
                recorded_year, duration_secs, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                expr.id.to_string(),
                expr.work_id.to_string(),
                expr.title,
                expr.musicbrainz_id,
                expr.conductor_id.map(|id| id.to_string()),
                expr.recorded_year.map(i64::from),
                expr.duration_secs,
                expr.created_at.to_rfc3339(),
                expr.updated_at.to_rfc3339(),
            ],
        )?;

        for performer_id in &expr.performer_ids {
            self.conn.execute(
                "INSERT INTO expression_performers (expression_id, artist_id)
                 VALUES (?1, ?2)",
                rusqlite::params![expr.id.to_string(), performer_id.to_string()],
            )?;
        }

        Ok(())
    }

    /// Insert or replace an expression and update its performer associations.
    pub fn upsert_expression(&self, expr: &Expression) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO expressions (
                id, work_id, title, musicbrainz_id, conductor_id,
                recorded_year, duration_secs, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                expr.id.to_string(),
                expr.work_id.to_string(),
                expr.title,
                expr.musicbrainz_id,
                expr.conductor_id.map(|id| id.to_string()),
                expr.recorded_year.map(i64::from),
                expr.duration_secs,
                expr.created_at.to_rfc3339(),
                expr.updated_at.to_rfc3339(),
            ],
        )?;

        // Replace all performer associations
        self.conn.execute(
            "DELETE FROM expression_performers WHERE expression_id = ?1",
            rusqlite::params![expr.id.to_string()],
        )?;

        for performer_id in &expr.performer_ids {
            self.conn.execute(
                "INSERT INTO expression_performers (expression_id, artist_id)
                 VALUES (?1, ?2)",
                rusqlite::params![expr.id.to_string(), performer_id.to_string()],
            )?;
        }

        Ok(())
    }

    /// List all expressions, including their performer IDs.
    pub fn list_expressions(&self) -> Result<Vec<Expression>> {
        use std::collections::HashMap;

        // Query 1: Get all expressions
        let mut stmt = self.conn.prepare(
            "SELECT id, work_id, title, musicbrainz_id, conductor_id,
                    recorded_year, duration_secs, created_at, updated_at
             FROM expressions
             ORDER BY title",
        )?;

        let mut expressions = stmt
            .query_map([], Self::row_to_expression)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Query 2: Get ALL performers for ALL expressions in one query (fixes N+1)
        let mut perf_stmt = self.conn.prepare(
            "SELECT expression_id, artist_id
             FROM expression_performers
             ORDER BY expression_id",
        )?;

        // Build a map: expression_id -> Vec<artist_id>
        let mut performers_map: HashMap<ExpressionId, Vec<ArtistId>> = HashMap::new();
        let rows = perf_stmt.query_map([], |row| {
            let expr_id_str: String = row.get(0)?;
            let artist_id_str: String = row.get(1)?;
            Ok((expr_id_str, artist_id_str))
        })?;

        for row_result in rows {
            let (expr_id_str, artist_id_str) = row_result?;

            let expr_id = uuid::Uuid::parse_str(&expr_id_str)
                .map(ExpressionId::from_uuid)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    0, rusqlite::types::Type::Text, Box::new(e)
                ))?;

            let artist_id = uuid::Uuid::parse_str(&artist_id_str)
                .map(ArtistId::from_uuid)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    1, rusqlite::types::Type::Text, Box::new(e)
                ))?;

            performers_map.entry(expr_id).or_default().push(artist_id);
        }

        // Populate performer IDs from the map
        for expr in &mut expressions {
            expr.performer_ids = performers_map.remove(&expr.id).unwrap_or_default();
        }

        Ok(expressions)
    }

    /// Look up an expression by its `MusicBrainz` ID, including performer IDs.
    pub fn get_expression_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Expression>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, work_id, title, musicbrainz_id, conductor_id,
                    recorded_year, duration_secs, created_at, updated_at
             FROM expressions
             WHERE musicbrainz_id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![mbid], Self::row_to_expression)?;

        match rows.next() {
            Some(row) => {
                let mut expr: Expression = row?;
                // Fetch performer IDs from junction table
                let mut perf_stmt = self.conn.prepare(
                    "SELECT artist_id FROM expression_performers
                     WHERE expression_id = ?1",
                )?;
                let performer_ids = perf_stmt
                    .query_map(rusqlite::params![expr.id.to_string()], |row| {
                        let id_str: String = row.get(0)?;
                        uuid::Uuid::parse_str(&id_str)
                            .map(ArtistId::from_uuid)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                                0, rusqlite::types::Type::Text, Box::new(e)
                            ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                expr.performer_ids = performer_ids;
                Ok(Some(expr))
            }
            None => Ok(None),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn row_to_expression(row: &rusqlite::Row) -> rusqlite::Result<Expression> {
        use chrono::DateTime;
        use uuid::Uuid;

        let id_str: String = row.get(0)?;
        let work_id_str: String = row.get(1)?;
        let conductor_id_str: Option<String> = row.get(4)?;
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        // Helper to parse optional UUID with proper error handling
        let parse_optional_artist_uuid = |s: Option<String>, col: usize| -> rusqlite::Result<Option<ArtistId>> {
            s.map(|val|
                Uuid::parse_str(&val)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        col, rusqlite::types::Type::Text, Box::new(e)
                    ))
                    .map(ArtistId::from_uuid)
            ).transpose()
        };

        Ok(Expression {
            id: ExpressionId::from_uuid(
                Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        0, rusqlite::types::Type::Text, Box::new(e)
                    ))?
            ),
            work_id: WorkId::from_uuid(
                Uuid::parse_str(&work_id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        1, rusqlite::types::Type::Text, Box::new(e)
                    ))?
            ),
            title: row.get(2)?,
            musicbrainz_id: row.get(3)?,
            performer_ids: Vec::new(), // populated after query
            conductor_id: parse_optional_artist_uuid(conductor_id_str, 4)?,
            recorded_year: row.get::<_, Option<i64>>(5)?.map(|v| v as i32),
            duration_secs: row.get(6)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    7, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    8, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
        })
    }
}

// Manifestation CRUD
impl Database {
    /// Insert a new manifestation.
    pub fn insert_manifestation(&self, man: &Manifestation) -> Result<()> {
        self.conn.execute(
            "INSERT INTO manifestations (
                id, title, musicbrainz_id, label, catalog_number,
                release_year, track_count, disc_count, format,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                man.id.to_string(),
                man.title,
                man.musicbrainz_id,
                man.label,
                man.catalog_number,
                man.release_year.map(i64::from),
                man.track_count.map(i64::from),
                man.disc_count.map(i64::from),
                man.format,
                man.created_at.to_rfc3339(),
                man.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Insert or replace a manifestation (upsert by primary key).
    pub fn upsert_manifestation(&self, man: &Manifestation) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO manifestations (
                id, title, musicbrainz_id, label, catalog_number,
                release_year, track_count, disc_count, format,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                man.id.to_string(),
                man.title,
                man.musicbrainz_id,
                man.label,
                man.catalog_number,
                man.release_year.map(i64::from),
                man.track_count.map(i64::from),
                man.disc_count.map(i64::from),
                man.format,
                man.created_at.to_rfc3339(),
                man.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Look up a manifestation by its `MusicBrainz` ID.
    pub fn get_manifestation_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Manifestation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, musicbrainz_id, label, catalog_number,
                    release_year, track_count, disc_count, format,
                    created_at, updated_at
             FROM manifestations
             WHERE musicbrainz_id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![mbid], Self::row_to_manifestation)?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn row_to_manifestation(row: &rusqlite::Row) -> rusqlite::Result<Manifestation> {
        use chrono::DateTime;
        use uuid::Uuid;

        let id_str: String = row.get(0)?;
        let created_at_str: String = row.get(9)?;
        let updated_at_str: String = row.get(10)?;

        Ok(Manifestation {
            id: ManifestationId::from_uuid(
                Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        0, rusqlite::types::Type::Text, Box::new(e)
                    ))?
            ),
            title: row.get(1)?,
            musicbrainz_id: row.get(2)?,
            label: row.get(3)?,
            catalog_number: row.get(4)?,
            release_year: row.get::<_, Option<i64>>(5)?.map(|v| v as i32),
            track_count: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
            disc_count: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
            format: row.get(8)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    9, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    10, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
        })
    }
}

// Artist CRUD
impl Database {
    /// Insert a new artist and its role associations.
    pub fn insert_artist(&self, artist: &Artist) -> Result<()> {
        self.conn.execute(
            "INSERT INTO artists (
                id, name, sort_name, musicbrainz_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                artist.id.to_string(),
                artist.name,
                artist.sort_name,
                artist.musicbrainz_id,
                artist.created_at.to_rfc3339(),
                artist.updated_at.to_rfc3339(),
            ],
        )?;

        for role in &artist.roles {
            self.conn.execute(
                "INSERT INTO artist_roles (artist_id, role) VALUES (?1, ?2)",
                rusqlite::params![artist.id.to_string(), format!("{role:?}")],
            )?;
        }

        Ok(())
    }

    /// Insert or replace an artist and update its role associations.
    pub fn upsert_artist(&self, artist: &Artist) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO artists (
                id, name, sort_name, musicbrainz_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                artist.id.to_string(),
                artist.name,
                artist.sort_name,
                artist.musicbrainz_id,
                artist.created_at.to_rfc3339(),
                artist.updated_at.to_rfc3339(),
            ],
        )?;

        // Replace all role associations
        self.conn.execute(
            "DELETE FROM artist_roles WHERE artist_id = ?1",
            rusqlite::params![artist.id.to_string()],
        )?;

        for role in &artist.roles {
            self.conn.execute(
                "INSERT INTO artist_roles (artist_id, role) VALUES (?1, ?2)",
                rusqlite::params![artist.id.to_string(), format!("{role:?}")],
            )?;
        }

        Ok(())
    }

    /// Look up an artist by its `MusicBrainz` ID, including roles.
    pub fn get_artist_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Artist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, sort_name, musicbrainz_id, created_at, updated_at
             FROM artists
             WHERE musicbrainz_id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![mbid], Self::row_to_artist)?;

        match rows.next() {
            Some(row) => {
                let mut artist: Artist = row?;
                // Fetch roles from junction table
                let mut role_stmt = self
                    .conn
                    .prepare("SELECT role FROM artist_roles WHERE artist_id = ?1")?;
                let roles = role_stmt
                    .query_map(rusqlite::params![artist.id.to_string()], |row| {
                        let role_str: String = row.get(0)?;
                        Ok(match role_str.as_str() {
                            "Composer" => ArtistRole::Composer,
                            "Performer" => ArtistRole::Performer,
                            "Conductor" => ArtistRole::Conductor,
                            "Ensemble" => ArtistRole::Ensemble,
                            "Producer" => ArtistRole::Producer,
                            _ => ArtistRole::Other,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                artist.roles = roles;
                Ok(Some(artist))
            }
            None => Ok(None),
        }
    }

    fn row_to_artist(row: &rusqlite::Row) -> rusqlite::Result<Artist> {
        use chrono::DateTime;
        use uuid::Uuid;

        let id_str: String = row.get(0)?;
        let created_at_str: String = row.get(4)?;
        let updated_at_str: String = row.get(5)?;

        Ok(Artist {
            id: ArtistId::from_uuid(
                Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        0, rusqlite::types::Type::Text, Box::new(e)
                    ))?
            ),
            name: row.get(1)?,
            sort_name: row.get(2)?,
            musicbrainz_id: row.get(3)?,
            roles: Vec::new(), // populated after query
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    4, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    5, rusqlite::types::Type::Text, Box::new(e)
                ))?
                .into(),
        })
    }
}

// LCGFT Vocabulary CRUD
impl Database {
    /// Insert a single LCGFT term.
    pub fn insert_lcgft_term(&self, term: &LcgftTerm) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO lcgft_terms (uri, label, broader_uri, scope_note, loaded_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![term.uri, term.label, term.broader_uri, term.scope_note,],
        )?;
        Ok(())
    }

    /// Look up an LCGFT term by its preferred label (case-insensitive).
    pub fn get_lcgft_by_label(&self, label: &str) -> Result<Option<LcgftTerm>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, label, broader_uri, scope_note
             FROM lcgft_terms
             WHERE label = ?1 COLLATE NOCASE",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![label], |row| {
            Ok(LcgftTerm {
                uri: row.get(0)?,
                label: row.get(1)?,
                broader_uri: row.get(2)?,
                scope_note: row.get(3)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Get all narrower (child) terms of a given LCGFT URI.
    pub fn get_lcgft_narrower(&self, uri: &str) -> Result<Vec<LcgftTerm>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, label, broader_uri, scope_note
             FROM lcgft_terms
             WHERE broader_uri = ?1
             ORDER BY label",
        )?;

        let terms = stmt
            .query_map(rusqlite::params![uri], |row| {
                Ok(LcgftTerm {
                    uri: row.get(0)?,
                    label: row.get(1)?,
                    broader_uri: row.get(2)?,
                    scope_note: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(terms)
    }

    /// Count the total number of LCGFT terms loaded.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn count_lcgft_terms(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM lcgft_terms", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

// LCMPT Vocabulary CRUD
impl Database {
    /// Insert a single LCMPT term.
    pub fn insert_lcmpt_term(&self, term: &LcmptTerm) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO lcmpt_terms (uri, label, broader_uri, scope_note, loaded_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![term.uri, term.label, term.broader_uri, term.scope_note,],
        )?;
        Ok(())
    }

    /// Look up an LCMPT term by its preferred label (case-insensitive).
    pub fn get_lcmpt_by_label(&self, label: &str) -> Result<Option<LcmptTerm>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, label, broader_uri, scope_note
             FROM lcmpt_terms
             WHERE label = ?1 COLLATE NOCASE",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![label], |row| {
            Ok(LcmptTerm {
                uri: row.get(0)?,
                label: row.get(1)?,
                broader_uri: row.get(2)?,
                scope_note: row.get(3)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Get all narrower (child) terms of a given LCMPT URI.
    pub fn get_lcmpt_narrower(&self, uri: &str) -> Result<Vec<LcmptTerm>> {
        let mut stmt = self.conn.prepare(
            "SELECT uri, label, broader_uri, scope_note
             FROM lcmpt_terms
             WHERE broader_uri = ?1
             ORDER BY label",
        )?;

        let terms = stmt
            .query_map(rusqlite::params![uri], |row| {
                Ok(LcmptTerm {
                    uri: row.get(0)?,
                    label: row.get(1)?,
                    broader_uri: row.get(2)?,
                    scope_note: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(terms)
    }

    /// Count the total number of LCMPT terms loaded.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn count_lcmpt_terms(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM lcmpt_terms", [], |row| row.get(0))?;
        Ok(count as usize)
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
        assert_eq!(count, 2); // Two migrations applied
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

    #[test]
    fn test_work_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let work = Work::new("String Quartet No. 4")
            .with_composer("Bela Bartok")
            .with_musicbrainz_id("test-mb-work-id")
            .with_catalog_number("Sz.91")
            .with_key("C major")
            .with_composed_year(1928);

        db.insert_work(&work).unwrap();

        let found = db.get_work_by_musicbrainz_id("test-mb-work-id").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.title, "String Quartet No. 4");
        assert_eq!(found.composer, Some("Bela Bartok".to_string()));
        assert_eq!(found.catalog_number, Some("Sz.91".to_string()));
        assert_eq!(found.key, Some("C major".to_string()));
        assert_eq!(found.composed_year, Some(1928));
    }

    #[test]
    fn test_artist_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let artist = Artist::new("Takacs Quartet")
            .with_role(ArtistRole::Ensemble)
            .with_musicbrainz_id("test-mb-artist-id");

        db.insert_artist(&artist).unwrap();

        let found = db
            .get_artist_by_musicbrainz_id("test-mb-artist-id")
            .unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.name, "Takacs Quartet");
        assert_eq!(found.roles, vec![ArtistRole::Ensemble]);
    }

    #[test]
    fn test_manifestation_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let man = Manifestation::new("String Quartets 1-6")
            .with_musicbrainz_id("test-mb-release-id")
            .with_label("Decca")
            .with_release_year(1998);

        db.insert_manifestation(&man).unwrap();

        let found = db
            .get_manifestation_by_musicbrainz_id("test-mb-release-id")
            .unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.title, "String Quartets 1-6");
        assert_eq!(found.label, Some("Decca".to_string()));
        assert_eq!(found.release_year, Some(1998));
    }

    #[test]
    fn test_expression_round_trip() {
        let db = Database::open_in_memory().unwrap();

        // Need a work and artist first (foreign keys)
        let work = Work::new("Test Work").with_musicbrainz_id("test-mb-work");
        db.insert_work(&work).unwrap();

        let artist = Artist::new("Test Performer").with_musicbrainz_id("test-mb-performer");
        db.insert_artist(&artist).unwrap();

        let expr = Expression::new(work.id)
            .with_title("Test Recording")
            .with_musicbrainz_id("test-mb-recording-id")
            .with_performer(artist.id)
            .with_duration(300.5);

        db.insert_expression(&expr).unwrap();

        let found = db
            .get_expression_by_musicbrainz_id("test-mb-recording-id")
            .unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.title, Some("Test Recording".to_string()));
        assert_eq!(found.work_id, work.id);
        assert_eq!(found.performer_ids.len(), 1);
        assert_eq!(found.performer_ids[0], artist.id);
        assert_eq!(found.duration_secs, Some(300.5));
    }

    #[test]
    fn test_work_upsert() {
        let db = Database::open_in_memory().unwrap();
        let mut work = Work::new("Original Title").with_musicbrainz_id("test-mb-upsert");
        db.insert_work(&work).unwrap();

        // Update title via upsert
        work.title = "Updated Title".to_string();
        db.upsert_work(&work).unwrap();

        let found = db
            .get_work_by_musicbrainz_id("test-mb-upsert")
            .unwrap()
            .unwrap();
        assert_eq!(found.title, "Updated Title");
    }

    #[test]
    fn test_list_all_items_empty() {
        let db = Database::open_in_memory().unwrap();
        let items = db.list_all_items().unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_list_all_items_returns_all() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();

        let item1 = Item::new(
            PathBuf::from("/music/alpha.flac"),
            AudioFormat::Flac,
            1024,
            now,
        );
        let item2 = Item::new(
            PathBuf::from("/music/beta.flac"),
            AudioFormat::Flac,
            2048,
            now,
        );

        db.insert_item(&item1).unwrap();
        db.insert_item(&item2).unwrap();

        let items = db.list_all_items().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_get_item_by_id_found() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();

        let item = Item::new(
            PathBuf::from("/music/test.flac"),
            AudioFormat::Flac,
            1024,
            now,
        );
        db.insert_item(&item).unwrap();

        let found = db.get_item_by_id(&item.id).unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id, item.id);
        assert_eq!(found.file_path, item.file_path);
    }

    #[test]
    fn test_get_item_by_id_not_found() {
        let db = Database::open_in_memory().unwrap();
        let missing_id = crate::model::ItemId::new();

        let found = db.get_item_by_id(&missing_id).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_list_identified_items_empty_when_none_identified() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();

        let item = Item::new(
            PathBuf::from("/music/test.flac"),
            AudioFormat::Flac,
            1024,
            now,
        );
        db.insert_item(&item).unwrap();

        let identified = db.list_identified_items().unwrap();
        assert!(identified.is_empty());
    }

    #[test]
    fn test_list_identified_items_returns_only_identified() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();

        // Create a work and expression for the identified item
        let work = Work::new("Test Work");
        db.insert_work(&work).unwrap();

        let expr = Expression::new(work.id).with_title("Test Recording");
        db.insert_expression(&expr).unwrap();

        // Unidentified item
        let item1 = Item::new(
            PathBuf::from("/music/unidentified.flac"),
            AudioFormat::Flac,
            1024,
            now,
        );
        db.insert_item(&item1).unwrap();

        // Identified item
        let mut item2 = Item::new(
            PathBuf::from("/music/identified.flac"),
            AudioFormat::Flac,
            2048,
            now,
        );
        item2.expression_id = Some(expr.id);
        db.insert_item(&item2).unwrap();

        let all = db.list_all_items().unwrap();
        assert_eq!(all.len(), 2);

        let identified = db.list_identified_items().unwrap();
        assert_eq!(identified.len(), 1);
        assert_eq!(identified[0].id, item2.id);

        let unidentified = db.list_unidentified_items().unwrap();
        assert_eq!(unidentified.len(), 1);
        assert_eq!(unidentified[0].id, item1.id);
    }

    #[test]
    fn test_list_expressions_empty() {
        let db = Database::open_in_memory().unwrap();
        let expressions = db.list_expressions().unwrap();
        assert!(expressions.is_empty());
    }

    #[test]
    fn test_list_expressions_with_performers() {
        let db = Database::open_in_memory().unwrap();

        let work = Work::new("Test Work");
        db.insert_work(&work).unwrap();

        let artist = Artist::new("Test Performer").with_musicbrainz_id("test-mb-perf");
        db.insert_artist(&artist).unwrap();

        let expr = Expression::new(work.id)
            .with_title("Test Recording")
            .with_musicbrainz_id("test-mb-expr")
            .with_performer(artist.id);
        db.insert_expression(&expr).unwrap();

        let expressions = db.list_expressions().unwrap();
        assert_eq!(expressions.len(), 1);
        assert_eq!(expressions[0].title, Some("Test Recording".to_string()));
        assert_eq!(expressions[0].performer_ids.len(), 1);
        assert_eq!(expressions[0].performer_ids[0], artist.id);
    }

    #[test]
    fn test_migration_002_creates_vocabulary_tables() {
        let db = Database::open_in_memory().unwrap();
        // Verify migration count (should be 2 now)
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_lcgft_term_round_trip() {
        let db = Database::open_in_memory().unwrap();

        // Insert parent term first (FK constraint)
        let parent = LcgftTerm::new(
            "http://id.loc.gov/authorities/genreForms/gf2014026090",
            "Chamber music",
        );
        db.insert_lcgft_term(&parent).unwrap();

        let term = LcgftTerm::new(
            "http://id.loc.gov/authorities/genreForms/gf2014026639",
            "String quartets",
        )
        .with_broader("http://id.loc.gov/authorities/genreForms/gf2014026090")
        .with_scope_note("Chamber music for two violins, viola, and cello");

        db.insert_lcgft_term(&term).unwrap();

        let found = db.get_lcgft_by_label("String quartets").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.uri, term.uri);
        assert_eq!(found.label, "String quartets");
        assert_eq!(found.broader_uri, term.broader_uri);
        assert_eq!(found.scope_note, term.scope_note);
    }

    #[test]
    fn test_lcgft_case_insensitive_lookup() {
        let db = Database::open_in_memory().unwrap();

        let term = LcgftTerm::new("http://example.com/gf1", "Art music");
        db.insert_lcgft_term(&term).unwrap();

        // Should find with different casing
        let found = db.get_lcgft_by_label("art music").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Art music");
    }

    #[test]
    fn test_lcgft_narrower_terms() {
        let db = Database::open_in_memory().unwrap();

        let parent = LcgftTerm::new("http://example.com/gf-parent", "Chamber music");
        let child1 = LcgftTerm::new("http://example.com/gf-child1", "String quartets")
            .with_broader("http://example.com/gf-parent");
        let child2 = LcgftTerm::new("http://example.com/gf-child2", "Piano trios")
            .with_broader("http://example.com/gf-parent");

        db.insert_lcgft_term(&parent).unwrap();
        db.insert_lcgft_term(&child1).unwrap();
        db.insert_lcgft_term(&child2).unwrap();

        let narrower = db
            .get_lcgft_narrower("http://example.com/gf-parent")
            .unwrap();
        assert_eq!(narrower.len(), 2);
        // Ordered by label
        assert_eq!(narrower[0].label, "Piano trios");
        assert_eq!(narrower[1].label, "String quartets");
    }

    #[test]
    fn test_lcgft_count() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_lcgft_terms().unwrap(), 0);

        db.insert_lcgft_term(&LcgftTerm::new("http://example.com/1", "Term 1"))
            .unwrap();
        db.insert_lcgft_term(&LcgftTerm::new("http://example.com/2", "Term 2"))
            .unwrap();
        assert_eq!(db.count_lcgft_terms().unwrap(), 2);
    }

    #[test]
    fn test_lcmpt_term_round_trip() {
        let db = Database::open_in_memory().unwrap();

        // Insert parent term first (FK constraint)
        let parent = LcmptTerm::new(
            "http://id.loc.gov/authorities/performanceMediums/mp2013015518",
            "bowed strings",
        );
        db.insert_lcmpt_term(&parent).unwrap();

        let term = LcmptTerm::new(
            "http://id.loc.gov/authorities/performanceMediums/mp2013015550",
            "violin",
        )
        .with_broader("http://id.loc.gov/authorities/performanceMediums/mp2013015518")
        .with_scope_note("A bowed string instrument");

        db.insert_lcmpt_term(&term).unwrap();

        let found = db.get_lcmpt_by_label("violin").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.uri, term.uri);
        assert_eq!(found.label, "violin");
        assert_eq!(found.broader_uri, term.broader_uri);
        assert_eq!(found.scope_note, term.scope_note);
    }

    #[test]
    fn test_lcmpt_narrower_terms() {
        let db = Database::open_in_memory().unwrap();

        let parent = LcmptTerm::new("http://example.com/mp-parent", "bowed strings");
        let child1 = LcmptTerm::new("http://example.com/mp-child1", "cello")
            .with_broader("http://example.com/mp-parent");
        let child2 = LcmptTerm::new("http://example.com/mp-child2", "violin")
            .with_broader("http://example.com/mp-parent");

        db.insert_lcmpt_term(&parent).unwrap();
        db.insert_lcmpt_term(&child1).unwrap();
        db.insert_lcmpt_term(&child2).unwrap();

        let narrower = db
            .get_lcmpt_narrower("http://example.com/mp-parent")
            .unwrap();
        assert_eq!(narrower.len(), 2);
        assert_eq!(narrower[0].label, "cello");
        assert_eq!(narrower[1].label, "violin");
    }

    #[test]
    fn test_lcmpt_count() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_lcmpt_terms().unwrap(), 0);

        db.insert_lcmpt_term(&LcmptTerm::new("http://example.com/1", "violin"))
            .unwrap();
        assert_eq!(db.count_lcmpt_terms().unwrap(), 1);
    }
}
