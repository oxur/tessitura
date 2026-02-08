/// A schema migration.
#[derive(Debug)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

const MIGRATION_001: &str = r#"
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Works (FRBR Work level)
CREATE TABLE IF NOT EXISTS works (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    composer TEXT,
    musicbrainz_id TEXT UNIQUE,
    catalog_number TEXT,
    key TEXT,
    composed_year INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_works_musicbrainz_id ON works(musicbrainz_id);
CREATE INDEX IF NOT EXISTS idx_works_composer ON works(composer);

-- Expressions (FRBR Expression level — performances/recordings)
CREATE TABLE IF NOT EXISTS expressions (
    id TEXT PRIMARY KEY,
    work_id TEXT NOT NULL REFERENCES works(id),
    title TEXT,
    musicbrainz_id TEXT UNIQUE,
    conductor_id TEXT REFERENCES artists(id),
    recorded_year INTEGER,
    duration_secs REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_expressions_work_id ON expressions(work_id);
CREATE INDEX IF NOT EXISTS idx_expressions_musicbrainz_id ON expressions(musicbrainz_id);

-- Manifestations (FRBR Manifestation level — releases)
CREATE TABLE IF NOT EXISTS manifestations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    musicbrainz_id TEXT UNIQUE,
    label TEXT,
    catalog_number TEXT,
    release_year INTEGER,
    track_count INTEGER,
    disc_count INTEGER,
    format TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_manifestations_musicbrainz_id ON manifestations(musicbrainz_id);

-- Items (FRBR Item level — audio files)
CREATE TABLE IF NOT EXISTS items (
    id TEXT PRIMARY KEY,
    expression_id TEXT REFERENCES expressions(id),
    manifestation_id TEXT REFERENCES manifestations(id),
    file_path TEXT NOT NULL UNIQUE,
    format TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    file_mtime TEXT NOT NULL,
    file_hash TEXT,
    fingerprint TEXT,
    fingerprint_score REAL,
    tag_title TEXT,
    tag_artist TEXT,
    tag_album TEXT,
    tag_album_artist TEXT,
    tag_track_number INTEGER,
    tag_disc_number INTEGER,
    tag_year INTEGER,
    tag_genre TEXT,
    duration_secs REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_items_expression_id ON items(expression_id);
CREATE INDEX IF NOT EXISTS idx_items_manifestation_id ON items(manifestation_id);
CREATE INDEX IF NOT EXISTS idx_items_file_path ON items(file_path);

-- Artists
CREATE TABLE IF NOT EXISTS artists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    sort_name TEXT,
    musicbrainz_id TEXT UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artists_musicbrainz_id ON artists(musicbrainz_id);

-- Artist roles (many-to-many: artist can have multiple roles)
CREATE TABLE IF NOT EXISTS artist_roles (
    artist_id TEXT NOT NULL REFERENCES artists(id),
    role TEXT NOT NULL,
    PRIMARY KEY (artist_id, role)
);

-- Expression performers (many-to-many)
CREATE TABLE IF NOT EXISTS expression_performers (
    expression_id TEXT NOT NULL REFERENCES expressions(id),
    artist_id TEXT NOT NULL REFERENCES artists(id),
    role TEXT,
    PRIMARY KEY (expression_id, artist_id)
);

-- Manifestation-Expression junction (a release contains recordings)
CREATE TABLE IF NOT EXISTS manifestation_expressions (
    manifestation_id TEXT NOT NULL REFERENCES manifestations(id),
    expression_id TEXT NOT NULL REFERENCES expressions(id),
    track_number INTEGER,
    disc_number INTEGER,
    PRIMARY KEY (manifestation_id, expression_id)
);

-- Provenance assertions
CREATE TABLE IF NOT EXISTS assertions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT NOT NULL,
    field TEXT NOT NULL,
    value TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL,
    fetched_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_assertions_entity_id ON assertions(entity_id);
CREATE INDEX IF NOT EXISTS idx_assertions_entity_field ON assertions(entity_id, field);
"#;

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: MIGRATION_001,
}];
