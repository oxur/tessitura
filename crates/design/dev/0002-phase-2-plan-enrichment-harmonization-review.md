# Phase 2 Implementation Plan: Enrichment + Harmonization + Review

## Context

Phase 1 is complete: the FRBR data model, SQLite schema, scan stage, identify stage, and CLI are working with 43 tests passing. The project plan (v1.1) defines Phase 2 as milestones 2.1–2.16 covering: fan-out enrichment from 5 external sources (MusicBrainz, Wikidata, Last.fm, LCGFT/LCMPT, Discogs), a mapping rules engine for harmonization, a ratatui-based review TUI, and audio fingerprinting for improved identification.

This plan will be written to `crates/design/dev/0002-phase-2-implementation-plan-enrichment-harmonization-review.md` and copied to `./workbench/`.

**Target:** Build the fan-out enrichment from multiple metadata sources, the
mapping/harmonization rules engine, and the human review TUI. Process test
albums through the full pipeline.

**Deliverable:** `tessitura enrich`, `tessitura harmonize`, `tessitura review`
working with real data. The user can review proposed metadata and approve/edit.

---

## Table of Contents

1. [Milestone 2.0: Complete Phase 1 Stubs](#milestone-20-complete-phase-1-stubs)
2. [Milestone 2.1: MusicBrainz Enrichment](#milestone-21-musicbrainz-enrichment)
3. [Milestone 2.2: Wikidata Enrichment](#milestone-22-wikidata-enrichment)
4. [Milestone 2.3: Last.fm Enrichment](#milestone-23-lastfm-enrichment)
5. [Milestone 2.4: LCGFT/LCMPT Loading](#milestone-24-lcgftlcmpt-loading)
6. [Milestone 2.5: Discogs Enrichment](#milestone-25-discogs-enrichment)
7. [Milestone 2.6: Enrich Stage (Fan-Out)](#milestone-26-enrich-stage-fan-out)
8. [Milestone 2.7: Mapping Rules Engine](#milestone-27-mapping-rules-engine)
9. [Milestone 2.8: Harmonize Stage](#milestone-28-harmonize-stage)
10. [Milestone 2.9: Review TUI](#milestone-29-review-tui)
11. [Milestone 2.10: Full Pipeline Wiring](#milestone-210-full-pipeline-wiring)
12. [Milestone 2.11: Audio Decoding Integration](#milestone-211-audio-decoding-integration)
13. [Milestone 2.12: Chromaprint Fingerprinting in Scan](#milestone-212-chromaprint-fingerprinting-in-scan)
14. [Milestone 2.13: AcoustID Lookup in Identify](#milestone-213-acoustid-lookup-in-identify)
15. [Milestone 2.14: Backfill Command for Existing Items](#milestone-214-backfill-command-for-existing-items)
16. [Milestone 2.15: Unified Process Command](#milestone-215-unified-process-command)
17. [Milestone 2.16: Mapping Rules Iteration](#milestone-216-mapping-rules-iteration)
18. [Appendix A: New Dependency Checklist](#appendix-a-new-dependency-checklist)
19. [Appendix B: External API Quick Reference](#appendix-b-external-api-quick-reference)
20. [Appendix C: Mapping Rules TOML Format](#appendix-c-mapping-rules-toml-format)
21. [Appendix D: New File Inventory](#appendix-d-new-file-inventory)

---

## Milestone 2.0: Complete Phase 1 Stubs

### Goal

Before starting enrichment, complete the `todo!()` stubs from Phase 1 that
enrichment depends on: the four entity insert/upsert methods in `Database`,
plus additional query methods needed by enrichment stages.

### Where

`crates/tessitura-core/src/schema/db.rs`

### Steps

#### 2.0.1: Implement `insert_work()`

Replace the `todo!()` with a real INSERT matching the works table schema:

```rust
pub fn insert_work(&self, work: &Work) -> Result<()> {
    self.conn.execute(
        "INSERT INTO works (id, title, composer, musicbrainz_id, catalog_number,
            key, composed_year, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            work.id.to_string(),
            work.title,
            work.composer,
            work.musicbrainz_id,
            work.catalog_number,
            work.key,
            work.composed_year,
            work.created_at.to_rfc3339(),
            work.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}
```

#### 2.0.2: Implement `insert_expression()`

```rust
pub fn insert_expression(&self, expr: &Expression) -> Result<()> {
    self.conn.execute(
        "INSERT INTO expressions (id, work_id, title, musicbrainz_id,
            conductor_id, recorded_year, duration_secs, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            expr.id.to_string(),
            expr.work_id.to_string(),
            expr.title,
            expr.musicbrainz_id,
            expr.conductor_id.map(|id| id.to_string()),
            expr.recorded_year,
            expr.duration_secs,
            expr.created_at.to_rfc3339(),
            expr.updated_at.to_rfc3339(),
        ],
    )?;
    // Insert performer links
    for performer_id in &expr.performer_ids {
        self.conn.execute(
            "INSERT OR IGNORE INTO expression_performers (expression_id, artist_id)
             VALUES (?1, ?2)",
            rusqlite::params![expr.id.to_string(), performer_id.to_string()],
        )?;
    }
    Ok(())
}
```

#### 2.0.3: Implement `insert_manifestation()` and `insert_artist()`

Follow the same pattern. Also implement upsert variants:

- `upsert_work(&self, work: &Work)` — INSERT OR REPLACE
- `upsert_expression(&self, expr: &Expression)` — INSERT OR REPLACE
- `upsert_manifestation(&self, man: &Manifestation)` — INSERT OR REPLACE
- `upsert_artist(&self, artist: &Artist)` — INSERT OR REPLACE
- `get_work_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Work>>`
- `get_expression_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Expression>>`
- `get_manifestation_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Manifestation>>`
- `get_artist_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Artist>>`

#### 2.0.4: Add tests for all new CRUD methods

Round-trip tests for each entity type (insert → query → verify fields).

### Acceptance Criteria

- [ ] All four `todo!()` stubs replaced with working implementations
- [ ] Upsert and query-by-musicbrainz-id methods work for all entity types
- [ ] Round-trip tests pass for Work, Expression, Manifestation, Artist
- [ ] `cargo test -p tessitura-core` passes

---

## Milestone 2.1: MusicBrainz Enrichment

### Goal

Expand the existing `MusicBrainzClient` to fetch full recording, release, and
work metadata. Create a `MusicBrainzEnricher` that stores assertions with
provenance. Rate-limited at 1 req/sec with exponential backoff retry and
circuit breaker.

### Where

- `crates/tessitura-etl/src/musicbrainz.rs` — expand existing client
- `crates/tessitura-etl/src/enrich/musicbrainz.rs` — enricher logic
- `crates/tessitura-etl/src/enrich/resilience.rs` — shared rate limiter

### Dependencies

Milestone 2.0

### Steps

#### 2.1.1: Create the enrichment error type

Create `crates/tessitura-etl/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnrichError {
    #[error("HTTP error from {source_name}: {message}")]
    Http { source_name: String, message: String },

    #[error("rate limited by {source_name}")]
    RateLimited { source_name: String },

    #[error("not found: {entity} at {source_name}")]
    NotFound { entity: String, source_name: String },

    #[error("parse error from {source_name}: {message}")]
    Parse { source_name: String, message: String },

    #[error("database error: {0}")]
    Database(#[from] tessitura_core::Error),

    #[error("circuit open for {source_name}")]
    CircuitOpen { source_name: String },
}

impl EnrichError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Http { .. } | Self::RateLimited { .. })
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }
}

pub type EnrichResult<T> = std::result::Result<T, EnrichError>;
```

#### 2.1.2: Create shared resilience utilities

Create `crates/tessitura-etl/src/enrich/resilience.rs`:

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{Duration, sleep};

/// Per-source rate limiter using a token bucket approach.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    interval: Duration,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            interval: Duration::from_millis(1000 / u64::from(requests_per_second)),
        }
    }

    pub async fn acquire(&self) {
        let _permit = self.semaphore.acquire().await.unwrap();
        sleep(self.interval).await;
    }
}
```

Use `backon` for retry logic with exponential backoff. Use `failsafe` for
circuit breaker per source.

#### 2.1.3: Expand MusicBrainz client with new endpoints

Add to existing `crates/tessitura-etl/src/musicbrainz.rs`:

- `get_work(&self, mbid: &str) -> Result<MbWorkDetail>` — fetch work with
  composer relationships, key, catalog attributes
- `get_release(&self, mbid: &str) -> Result<MbReleaseDetail>` — fetch release
  with labels, media, track listing
- `search_recording(&self, artist: &str, title: &str, album: &str) -> Result<Vec<MbRecording>>`
  — Lucene query search fallback

Add response types:

```rust
#[derive(Debug, Deserialize)]
pub struct MbWorkDetail {
    pub id: String,
    pub title: String,
    pub relations: Vec<MbRelation>,
    pub attributes: Vec<String>,  // includes key like "A minor"
}

#[derive(Debug, Deserialize)]
pub struct MbReleaseDetail {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(rename = "label-info", default)]
    pub label_info: Vec<MbLabelInfo>,
    #[serde(default)]
    pub media: Vec<MbMedia>,
}

#[derive(Debug, Deserialize)]
pub struct MbLabelInfo {
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
    pub label: Option<MbLabel>,
}

#[derive(Debug, Deserialize)]
pub struct MbLabel {
    pub id: String,
    pub name: String,
}
```

#### 2.1.4: Create the MusicBrainz enricher

Create `crates/tessitura-etl/src/enrich/musicbrainz.rs`:

```rust
use crate::enrich::resilience::RateLimiter;
use crate::error::EnrichResult;
use crate::musicbrainz::MusicBrainzClient;
use tessitura_core::provenance::{Assertion, Source};
use tessitura_core::schema::Database;

#[derive(Debug, Clone)]
pub struct MusicBrainzEnricher {
    client: MusicBrainzClient,
    rate_limiter: RateLimiter,
}

impl MusicBrainzEnricher {
    pub fn new() -> EnrichResult<Self> {
        Ok(Self {
            client: MusicBrainzClient::new()?,
            rate_limiter: RateLimiter::new(1),  // 1 req/sec
        })
    }

    /// Enrich a single item: fetch recording, work, release details
    /// and store assertions in the database.
    pub async fn enrich(&self, item_id: &str, db: &Database) -> EnrichResult<Vec<Assertion>> {
        // 1. Get item from DB, find its musicbrainz recording ID
        // 2. Fetch recording details (already have from identify)
        // 3. If recording has work relations, fetch work details
        //    → store assertions: composer, catalog_number, key
        // 4. For each release, fetch release details
        //    → store assertions: label, catalog_number, release_year
        // 5. Store all assertions with Source::MusicBrainz
        todo!()
    }
}
```

Each assertion is stored via `db.insert_assertion()` with
`Source::MusicBrainz` and appropriate confidence scores.

#### 2.1.5: Tests with mocked HTTP

Use `mockito` or `wiremock` to test the enricher against canned MB API
responses. Test both success and error paths (rate limit, 404, malformed JSON).

### Acceptance Criteria

- [ ] `MusicBrainzEnricher` fetches recording, work, and release details
- [ ] Assertions are stored with `Source::MusicBrainz` provenance
- [ ] Rate limiting enforced at 1 req/sec
- [ ] Retry logic handles 503 (rate limited) responses
- [ ] Circuit breaker opens after repeated failures
- [ ] Tests pass with mocked HTTP responses
- [ ] Errors include context (which MB ID, which endpoint)

---

## Milestone 2.2: Wikidata Enrichment

### Goal

Query Wikidata for structured properties linked via MusicBrainz work IDs.
Extract: musical key, form, opus/catalog number, instrumentation, period,
school/movement.

### Where

`crates/tessitura-etl/src/enrich/wikidata.rs`

### Dependencies

Milestone 2.0

### Steps

#### 2.2.1: Create the Wikidata client

```rust
#[derive(Debug, Clone)]
pub struct WikidataClient {
    http: reqwest::Client,
    rate_limiter: RateLimiter,
}

impl WikidataClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent("tessitura/0.1.0 (https://github.com/oxur/tessitura)")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            rate_limiter: RateLimiter::new(5),  // ~5 req/sec
        }
    }

    /// Find Wikidata QID for a MusicBrainz work ID.
    pub async fn find_by_mb_work_id(&self, mb_work_id: &str) -> EnrichResult<Option<String>> {
        self.rate_limiter.acquire().await;
        let query = format!(
            r#"SELECT ?item WHERE {{ ?item wdt:P435 "{}" }} LIMIT 1"#,
            mb_work_id
        );
        // POST to https://query.wikidata.org/sparql with query param
        // Parse SPARQL JSON results
        todo!()
    }

    /// Fetch properties for a Wikidata QID.
    pub async fn get_properties(&self, qid: &str) -> EnrichResult<WikidataProperties> {
        self.rate_limiter.acquire().await;
        // GET https://www.wikidata.org/wiki/Special:EntityData/{qid}.json
        // Extract relevant properties
        todo!()
    }
}
```

#### 2.2.2: Define Wikidata property mappings

Key Wikidata properties for music works:

| Property | ID | Maps To |
|---|---|---|
| Tonality | P826 | Work.key |
| Form of creative work | P7937 | Form taxonomy |
| Catalog code | P528 | Work.catalog_number |
| Instrumentation | P870 | Instrumentation taxonomy |
| Time period | P2348 | Period taxonomy |
| Movement | P135 | School/movement |
| MusicBrainz work ID | P435 | Used for linking |

#### 2.2.3: Create the Wikidata enricher

```rust
pub struct WikidataEnricher {
    client: WikidataClient,
}

impl WikidataEnricher {
    pub async fn enrich(&self, item_id: &str, db: &Database) -> EnrichResult<Vec<Assertion>> {
        // 1. Get item → expression → work → musicbrainz_id
        // 2. Find Wikidata QID via P435 (MB work ID)
        // 3. Fetch properties
        // 4. Store assertions:
        //    - key (P826) → field "key", Source::Wikidata
        //    - form (P7937) → field "form", Source::Wikidata
        //    - catalog (P528) → field "catalog_number", Source::Wikidata
        //    - instrumentation (P870) → field "instrumentation", Source::Wikidata
        //    - period (P2348) → field "period", Source::Wikidata
        //    - school (P135) → field "school", Source::Wikidata
        todo!()
    }
}
```

### Acceptance Criteria

- [ ] Wikidata QID lookup by MusicBrainz work ID works
- [ ] Key, form, catalog, instrumentation, period, school extracted
- [ ] Assertions stored with `Source::Wikidata`
- [ ] Rate limiting at ~5 req/sec
- [ ] Graceful handling when no Wikidata entry exists
- [ ] Tests with mocked SPARQL and REST responses

---

## Milestone 2.3: Last.fm Enrichment

### Goal

Fetch folksonomy tags for artist and track from Last.fm. These capture
subjective/aesthetic qualities that formal taxonomies miss (e.g., "dark,"
"atmospheric," "haunting").

### Where

`crates/tessitura-etl/src/enrich/lastfm.rs`

### Dependencies

Milestone 2.0

### Steps

#### 2.3.1: Create the Last.fm client

```rust
#[derive(Debug, Clone)]
pub struct LastFmClient {
    http: reqwest::Client,
    api_key: String,
    rate_limiter: RateLimiter,
}

impl LastFmClient {
    pub fn new(api_key: String) -> Self { ... }

    pub async fn get_track_tags(
        &self, artist: &str, track: &str
    ) -> EnrichResult<Vec<LastFmTag>> {
        self.rate_limiter.acquire().await;
        // GET ?method=track.getTopTags&artist={}&track={}&api_key={}&format=json
        todo!()
    }

    pub async fn get_artist_tags(
        &self, artist: &str
    ) -> EnrichResult<Vec<LastFmTag>> {
        self.rate_limiter.acquire().await;
        // GET ?method=artist.getTopTags&artist={}&api_key={}&format=json
        todo!()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastFmTag {
    pub name: String,
    pub count: u32,  // usage count / weight
}
```

#### 2.3.2: Create the Last.fm enricher

Store each tag above a minimum count threshold as an assertion with
`Source::LastFm`. The tag count maps to confidence (normalized 0.0–1.0).

#### 2.3.3: Add `lastfm_api_key` to Config

Update `crates/tessitura-etl/src/config.rs` to include:

```rust
/// Last.fm API key for folksonomy tag enrichment.
#[serde(default)]
pub lastfm_api_key: Option<String>,
```

### Acceptance Criteria

- [ ] Track and artist tags fetched from Last.fm
- [ ] Tags stored as assertions with normalized confidence
- [ ] Low-count tags filtered out (configurable threshold)
- [ ] Rate limiting at 5 req/sec
- [ ] Config support for `TESS_LASTFM_API_KEY`

---

## Milestone 2.4: LCGFT/LCMPT Loading

### Goal

Ingest Library of Congress controlled vocabularies from SKOS/RDF snapshots.
Build hierarchical term relationships (broader/narrower). These provide the
authoritative genre/form and instrumentation vocabularies that mapping rules
target.

### Where

- `crates/tessitura-core/src/taxonomy/genre.rs` — add `LcgftTerm` type
- `crates/tessitura-core/src/taxonomy/instrumentation.rs` — add `LcmptTerm`
- `crates/tessitura-core/src/schema/migrations.rs` — migration 002
- `crates/tessitura-core/src/schema/db.rs` — vocabulary CRUD
- `crates/tessitura-etl/src/enrich/lcgft.rs` — loader

### Dependencies

Milestone 1.4 (taxonomy stubs)

### Steps

#### 2.4.1: Add vocabulary tables (migration 002)

Add to `crates/tessitura-core/src/schema/migrations.rs`:

```rust
Migration {
    version: 2,
    name: "vocabulary_tables",
    sql: MIGRATION_002,
},
```

```sql
-- LCGFT terms (genre/form controlled vocabulary)
CREATE TABLE IF NOT EXISTS lcgft_terms (
    uri TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    broader_uri TEXT REFERENCES lcgft_terms(uri),
    scope_note TEXT,
    loaded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_lcgft_label ON lcgft_terms(label);
CREATE INDEX IF NOT EXISTS idx_lcgft_broader ON lcgft_terms(broader_uri);

-- LCMPT terms (instrumentation controlled vocabulary)
CREATE TABLE IF NOT EXISTS lcmpt_terms (
    uri TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    broader_uri TEXT REFERENCES lcmpt_terms(uri),
    scope_note TEXT,
    loaded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_lcmpt_label ON lcmpt_terms(label);
CREATE INDEX IF NOT EXISTS idx_lcmpt_broader ON lcmpt_terms(broader_uri);
```

#### 2.4.2: Define vocabulary types

Add `LcgftTerm` to `crates/tessitura-core/src/taxonomy/genre.rs`:

```rust
/// A Library of Congress Genre/Form Term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcgftTerm {
    pub uri: String,
    pub label: String,
    pub broader_uri: Option<String>,
    pub scope_note: Option<String>,
}
```

Add `LcmptTerm` to `crates/tessitura-core/src/taxonomy/instrumentation.rs`:

```rust
/// A Library of Congress Medium of Performance Term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcmptTerm {
    pub uri: String,
    pub label: String,
    pub broader_uri: Option<String>,
    pub scope_note: Option<String>,
}
```

#### 2.4.3: Add vocabulary CRUD to Database

Add methods to `db.rs`:

- `insert_lcgft_term(&self, term: &LcgftTerm) -> Result<()>`
- `get_lcgft_by_label(&self, label: &str) -> Result<Option<LcgftTerm>>`
- `get_lcgft_broader(&self, uri: &str) -> Result<Vec<LcgftTerm>>`
- Same for LCMPT

#### 2.4.4: Create the vocabulary loader

Create `crates/tessitura-etl/src/enrich/lcgft.rs`:

```rust
/// Load LCGFT terms from a JSON-LD/SKOS snapshot file.
pub fn load_lcgft(db: &Database, snapshot_path: &Path) -> Result<usize> {
    // Parse JSON-LD, extract concepts with labels and broader relations
    // Insert into lcgft_terms table
    // Return count of terms loaded
    todo!()
}

/// Load LCMPT terms from a JSON-LD/SKOS snapshot file.
pub fn load_lcmpt(db: &Database, snapshot_path: &Path) -> Result<usize> {
    todo!()
}
```

The LC vocabularies are published as linked data. We'll ship JSON snapshots
in `config/lcgft-snapshot.json` and `config/lcmpt-snapshot.json`, with a CLI
command to refresh them.

#### 2.4.5: Add `tessitura vocab load` CLI command

```rust
Commands::Vocab { action: VocabAction },

enum VocabAction {
    /// Load LCGFT/LCMPT vocabulary snapshots into the database
    Load,
    /// Show vocabulary statistics
    Stats,
}
```

### Acceptance Criteria

- [ ] Migration 002 creates vocabulary tables
- [ ] LCGFT terms load from JSON snapshot with broader/narrower hierarchy
- [ ] LCMPT terms load from JSON snapshot
- [ ] `tessitura vocab load` populates both vocabularies
- [ ] Label-based lookup works (`get_lcgft_by_label("String quartets")`)
- [ ] Hierarchy traversal works (find all narrower terms of "Art music")
- [ ] Tests with fixture data

---

## Milestone 2.5: Discogs Enrichment

### Goal

Fetch release-level detail from Discogs: label, pressing info, personnel
credits. Match via MusicBrainz release ID or catalog number search.

### Where

`crates/tessitura-etl/src/enrich/discogs.rs`

### Dependencies

Milestone 2.0

### Steps

#### 2.5.1: Create the Discogs client

```rust
#[derive(Debug, Clone)]
pub struct DiscogsClient {
    http: reqwest::Client,
    token: Option<String>,
    rate_limiter: RateLimiter,
}

impl DiscogsClient {
    pub fn new(token: Option<String>) -> Self {
        let rps = if token.is_some() { 4 } else { 1 };  // 240/min vs 60/min
        Self {
            http: reqwest::Client::builder()
                .user_agent("tessitura/0.1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            token,
            rate_limiter: RateLimiter::new(rps),
        }
    }

    pub async fn search_release(&self, catno: &str) -> EnrichResult<Vec<DiscogsRelease>> {
        // GET /database/search?catno={catno}&type=release
        todo!()
    }

    pub async fn get_release(&self, id: u64) -> EnrichResult<DiscogsReleaseDetail> {
        // GET /releases/{id}
        todo!()
    }
}
```

#### 2.5.2: Create the Discogs enricher

Store assertions for: label, format (CD/LP/Digital), personnel credits
(extra artists), genres, styles. Use `Source::Discogs`.

#### 2.5.3: Add `discogs_token` to Config

```rust
/// Discogs personal access token (optional, increases rate limit).
#[serde(default)]
pub discogs_token: Option<String>,
```

### Acceptance Criteria

- [ ] Discogs release search by catalog number works
- [ ] Release details extracted: label, format, personnel, genres, styles
- [ ] Assertions stored with `Source::Discogs`
- [ ] Rate limiting: 1 req/sec unauthenticated, 4 req/sec with token
- [ ] Config support for `TESS_DISCOGS_TOKEN`

---

## Milestone 2.6: Enrich Stage (Fan-Out)

### Goal

Implement the treadle `Stage` that orchestrates concurrent enrichment from all
five sources using treadle's `StageOutcome::FanOut` mechanism. Each source is a
tracked subtask with independent retry.

### Where

- `crates/tessitura-etl/src/enrich/mod.rs` — module declarations
- `crates/tessitura-etl/src/enrich/stage.rs` — `EnrichStage`

### Dependencies

Milestones 2.1–2.5

### Steps

#### 2.6.1: Create the enrich module structure

```
crates/tessitura-etl/src/enrich/
├── mod.rs
├── stage.rs          # EnrichStage (fan-out treadle Stage)
├── resilience.rs     # RateLimiter, shared utilities
├── musicbrainz.rs    # MusicBrainzEnricher
├── wikidata.rs       # WikidataEnricher
├── lastfm.rs         # LastFmEnricher
├── lcgft.rs          # Vocabulary loader (not a subtask)
└── discogs.rs        # DiscogsEnricher
```

#### 2.6.2: Implement the EnrichStage

The stage uses treadle's fan-out pattern:

```rust
use treadle::{Stage, StageContext, StageOutcome, SubTask, WorkItem};

#[derive(Debug)]
pub struct EnrichStage {
    musicbrainz: Option<MusicBrainzEnricher>,
    wikidata: Option<WikidataEnricher>,
    lastfm: Option<LastFmEnricher>,
    discogs: Option<DiscogsEnricher>,
    db_path: PathBuf,
}

#[async_trait::async_trait]
impl Stage for EnrichStage {
    fn name(&self) -> &str { "enrich" }

    async fn execute(
        &self,
        item: &dyn WorkItem,
        ctx: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        match ctx.subtask_name.as_deref() {
            // First call: fan out to all enabled sources
            None => {
                let mut subtasks = Vec::new();
                if self.musicbrainz.is_some() {
                    subtasks.push(SubTask::new("musicbrainz".to_string()));
                }
                if self.wikidata.is_some() {
                    subtasks.push(SubTask::new("wikidata".to_string()));
                }
                if self.lastfm.is_some() {
                    subtasks.push(SubTask::new("lastfm".to_string()));
                }
                if self.discogs.is_some() {
                    subtasks.push(SubTask::new("discogs".to_string()));
                }
                Ok(StageOutcome::FanOut(subtasks))
            }
            // Subtask calls: dispatch to the appropriate enricher
            Some("musicbrainz") => {
                let db = Database::open(&self.db_path)?;
                self.musicbrainz.as_ref().unwrap()
                    .enrich(item.id(), &db).await?;
                Ok(StageOutcome::Complete)
            }
            Some("wikidata") => {
                let db = Database::open(&self.db_path)?;
                self.wikidata.as_ref().unwrap()
                    .enrich(item.id(), &db).await?;
                Ok(StageOutcome::Complete)
            }
            Some("lastfm") => {
                let db = Database::open(&self.db_path)?;
                self.lastfm.as_ref().unwrap()
                    .enrich(item.id(), &db).await?;
                Ok(StageOutcome::Complete)
            }
            Some("discogs") => {
                let db = Database::open(&self.db_path)?;
                self.discogs.as_ref().unwrap()
                    .enrich(item.id(), &db).await?;
                Ok(StageOutcome::Complete)
            }
            Some(other) => {
                Err(treadle::Error::stage(format!("Unknown subtask: {}", other)))
            }
        }
    }
}
```

**Key design points:**

- Each enricher is `Option` — sources are enabled/disabled based on
  available API keys and config
- Fan-out subtasks are tracked independently by treadle — if MusicBrainz
  succeeds but Discogs fails, only Discogs is retried
- Database connections are opened per-subtask (rusqlite is `Send` but
  not `Sync`)

#### 2.6.3: Add the `tessitura enrich` CLI command

```rust
Commands::Enrich {
    /// Only enrich items that haven't been enriched yet
    #[arg(long, default_value_t = false)]
    pending_only: bool,
},
```

### Acceptance Criteria

- [ ] `EnrichStage` implements `treadle::Stage` with `FanOut`
- [ ] All enabled sources execute as independent subtasks
- [ ] Failed subtasks can be retried without re-running successful ones
- [ ] `tessitura enrich` triggers the enrichment pipeline
- [ ] Progress events show per-source status
- [ ] Integration test: mock all 4 APIs, verify assertions created

---

## Milestone 2.7: Mapping Rules Engine

### Goal

Build a configurable rules engine for normalizing genres, forms, periods, and
instrumentation from raw assertions into canonical controlled vocabulary terms.
Rules are stored in TOML, with priority ordering for conflict resolution.

### Where

- `crates/tessitura-core/src/taxonomy/rules.rs` — rule types and engine
- `config/taxonomy.toml` — default mapping rules

### Dependencies

Milestone 2.4 (LCGFT/LCMPT vocabulary loaded)

### Steps

#### 2.7.1: Define the mapping rules types

Create `crates/tessitura-core/src/taxonomy/rules.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete set of mapping rules loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRules {
    /// Source priority: source_name → priority (higher wins)
    pub source_priority: HashMap<String, u32>,

    /// Genre/form normalization rules
    #[serde(default)]
    pub genre_rules: Vec<GenreRule>,

    /// Period inference rules
    #[serde(default)]
    pub period_rules: Vec<PeriodRule>,

    /// Instrumentation normalization rules
    #[serde(default)]
    pub instrument_rules: Vec<InstrumentRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreRule {
    pub name: String,
    pub description: Option<String>,
    /// Match if the assertion value contains any of these strings (case-insensitive)
    pub match_any: Vec<String>,
    /// Only match assertions from these sources (empty = match all)
    #[serde(default)]
    pub match_source: Vec<String>,
    /// Output: normalized genre string (hierarchical with " > ")
    pub output_genre: Option<String>,
    /// Output: normalized form string
    pub output_form: Option<String>,
    /// Output: LCGFT label for linking to controlled vocabulary
    pub output_lcgft_label: Option<String>,
    /// Confidence of this rule's output (0.0-1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodRule {
    pub name: String,
    pub description: Option<String>,
    /// Match by composer name (case-insensitive contains)
    #[serde(default)]
    pub match_composer: Vec<String>,
    pub output_period: String,
    /// Composition year range for this period
    pub year_range: Option<[i32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentRule {
    pub name: String,
    pub description: Option<String>,
    pub match_any: Vec<String>,
    pub output_instruments: Vec<String>,
    #[serde(default)]
    pub output_lcmpt_labels: Vec<String>,
}

fn default_confidence() -> f64 { 0.8 }

impl MappingRules {
    /// Load rules from a TOML file.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules: Self = toml::from_str(&content)?;
        Ok(rules)
    }

    /// Get the priority for a source (default 0 if not configured).
    pub fn priority_for(&self, source: &str) -> u32 {
        self.source_priority.get(source).copied().unwrap_or(0)
    }
}
```

#### 2.7.2: Implement the rule matching logic

```rust
impl MappingRules {
    /// Apply genre rules to a set of assertions, returning proposed tags.
    pub fn apply_genre_rules(
        &self,
        assertions: &[Assertion],
    ) -> Vec<ProposedTag> {
        // For each assertion with field "genre":
        //   1. Try each genre_rule in order
        //   2. If match_any matches (case-insensitive) and match_source matches
        //   3. Record the proposed genre/form with rule confidence
        // Return all proposals; caller resolves conflicts
        todo!()
    }

    /// Apply period rules based on composer and composition year.
    pub fn apply_period_rules(
        &self,
        composer: Option<&str>,
        year: Option<i32>,
    ) -> Option<ProposedTag> {
        // Try each period_rule:
        //   1. If composer matches match_composer (substring, case-insensitive)
        //   2. Or if year falls within year_range
        todo!()
    }
}
```

#### 2.7.3: Define ProposedTag type

```rust
/// A proposed metadata value produced by the rules engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedTag {
    pub field: String,         // "genre", "form", "period", "instrumentation"
    pub value: String,         // Canonical value
    pub source: Source,        // Which source's assertion triggered this
    pub rule_name: String,     // Which rule produced this
    pub confidence: f64,       // Combined confidence
    pub alternatives: Vec<Alternative>,  // Other proposals that conflicted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub value: String,
    pub source: Source,
    pub confidence: f64,
}
```

#### 2.7.4: Create default `config/taxonomy.toml`

Ship a default mapping rules file covering the user's genres: classical
(with period subdivisions), jazz, electronic, prog rock. See
[Appendix C](#appendix-c-mapping-rules-toml-format) for the full format.

#### 2.7.5: Tests for rule matching

Test each rule type against sample assertions. Verify: correct matching,
priority ordering, confidence propagation, conflict detection.

### Acceptance Criteria

- [ ] `MappingRules` loads from TOML
- [ ] Genre rules match assertions by value and source
- [ ] Period rules match by composer name and year range
- [ ] Instrumentation rules map strings to standardized terms
- [ ] Source priority ordering works for conflict resolution
- [ ] `ProposedTag` captures alternatives for conflict display
- [ ] Default `config/taxonomy.toml` covers classical/jazz/electronic/prog
- [ ] Unit tests for all rule types

---

## Milestone 2.8: Harmonize Stage

### Goal

Implement the treadle `Stage` that applies mapping rules to enrichment
assertions, resolves conflicts using source priority, flags ambiguities,
and returns `StageOutcome::NeedsReview`.

### Where

`crates/tessitura-etl/src/harmonize.rs`

### Dependencies

Milestones 2.6 (enrichment assertions exist), 2.7 (rules engine)

### Steps

#### 2.8.1: Implement the HarmonizeStage

```rust
#[derive(Debug)]
pub struct HarmonizeStage {
    rules: MappingRules,
    db_path: PathBuf,
}

#[async_trait::async_trait]
impl Stage for HarmonizeStage {
    fn name(&self) -> &str { "harmonize" }

    async fn execute(
        &self,
        item: &dyn WorkItem,
        ctx: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        let db = Database::open(&self.db_path)?;

        // 1. Load all assertions for this item's entity IDs
        let assertions = db.get_assertions_for_entity(item.id())?;

        // 2. Apply genre rules → proposed genre + form tags
        let genre_proposals = self.rules.apply_genre_rules(&assertions);

        // 3. Apply period rules (using composer + year from assertions)
        let period_proposal = self.rules.apply_period_rules(
            composer.as_deref(), year
        );

        // 4. Apply instrument rules
        let instrument_proposals = self.rules.apply_instrument_rules(&assertions);

        // 5. Resolve conflicts using source priority
        let resolved = self.resolve_conflicts(
            &genre_proposals, &period_proposal, &instrument_proposals
        );

        // 6. Store resolved proposals in context metadata
        ctx.metadata.insert(
            "proposed_tags".to_string(),
            serde_json::to_value(&resolved)?,
        );

        // 7. Return NeedsReview so pipeline pauses for human approval
        Ok(StageOutcome::NeedsReview)
    }
}
```

#### 2.8.2: Conflict resolution logic

```rust
impl HarmonizeStage {
    fn resolve_conflicts(&self, proposals: &[ProposedTag]) -> Vec<ProposedTag> {
        // Group proposals by field
        // For each field:
        //   - If only one proposal, use it
        //   - If multiple proposals agree, merge (highest confidence)
        //   - If multiple proposals disagree, select by source priority
        //     but record alternatives for review display
        todo!()
    }
}
```

#### 2.8.3: Add `tessitura harmonize` CLI command

```rust
Commands::Harmonize,
```

Runs harmonization on all enriched items that haven't been harmonized yet.

### Acceptance Criteria

- [ ] `HarmonizeStage` applies rules and resolves conflicts
- [ ] Proposed tags stored in treadle context metadata
- [ ] Returns `NeedsReview` so pipeline pauses
- [ ] Conflicts include alternatives with source provenance
- [ ] `tessitura harmonize` triggers harmonization
- [ ] Test: multiple conflicting assertions → correct resolution

---

## Milestone 2.9: Review TUI

### Goal

Build a ratatui-based terminal UI for human review of proposed metadata.
Two-level navigation: album list → track detail drill-down. Show proposed
tags, conflicts with resolution rationale, source provenance. Support
accept all / track-by-track review / edit rules / skip.

### Where

- `crates/tessitura-cli/src/tui/mod.rs` — TUI app state and event loop
- `crates/tessitura-cli/src/tui/album_list.rs` — album list view
- `crates/tessitura-cli/src/tui/track_detail.rs` — track detail view
- `crates/tessitura-cli/src/commands/review.rs` — CLI entry point

### Dependencies

Milestone 2.8

### Steps

#### 2.9.1: Add ratatui and crossterm dependencies

Add to workspace `Cargo.toml`:

```toml
ratatui = "0.29"
crossterm = "0.28"
```

Add to `crates/tessitura-cli/Cargo.toml`:

```toml
ratatui = { workspace = true }
crossterm = { workspace = true }
```

#### 2.9.2: Define TUI app state

```rust
pub struct App {
    pub view: View,
    pub albums: Vec<ReviewAlbum>,
    pub selected_album: usize,
    pub selected_track: usize,
    pub should_quit: bool,
    pub db: Database,
    pub workflow: Workflow,
    pub store: SqliteStateStore,
}

pub enum View {
    AlbumList,
    TrackDetail(usize),  // album index
}

pub struct ReviewAlbum {
    pub title: String,
    pub artist: String,
    pub tracks: Vec<ReviewTrack>,
    pub conflict_count: usize,
}

pub struct ReviewTrack {
    pub item_id: String,
    pub title: String,
    pub proposed_tags: Vec<ProposedTag>,
    pub has_conflicts: bool,
}
```

#### 2.9.3: Implement the album list view

Render a table matching the project plan's review workflow mockup:

```
┌───────────────────────────────────────────────────────────┐
│  Albums Awaiting Review                           12 new  │
├────┬────────────────────────┬───────────────┬─────────────┤
│  # │ Album                  │ Artist        │ Status      │
├────┼────────────────────────┼───────────────┼─────────────┤
│  1 │ String Quartets 1-6    │ Bartók/Takács │ 6/6 pending │
│ ...│ ...                    │               │             │
└────┴────────────────────────┴───────────────┴─────────────┘
```

Navigation: `j/k` or `↑/↓` to select, `Enter` to drill into album,
`a` to accept all, `q` to quit.

#### 2.9.4: Implement the track detail view

Show per-track proposed tags with conflict details:

```
┌─────────────────────────────────────────────────────────────────┐
│  String Quartets 1-6 — Bartók — Takács Quartet                 │
│  Track 1: SQ No.1, I. Lento                                    │
├─────────────────────────────────────────────────────────────────┤
│  Proposed Tags:                                                 │
│    genre: Classical > 20th Century                              │
│    form: String quartet                                         │
│    key: A minor                              [Wikidata ✓]       │
│    instrumentation: 2vn, va, vc              [Wikidata ✓]       │
│    catalog: Sz.40, BB.52                     [MusicBrainz ✓]    │
│                                                                 │
│  Conflicts:                                                     │
│    genre: MB="Classical" Last.fm="modern classical, 20th c."    │
│           LCGFT="String quartets"                               │
│           → resolved: Classical > 20th Century (rule: ...)      │
├─────────────────────────────────────────────────────────────────┤
│  [a]ccept  [e]dit  [n]ext  [p]rev  [b]ack  [q]uit              │
└─────────────────────────────────────────────────────────────────┘
```

#### 2.9.5: Implement review actions

When the user accepts a track/album:

1. Call `workflow.approve_review(item_id, "harmonize", &mut store)` via
   treadle's review approval API
2. Update the item's metadata in the database with the accepted proposed tags
3. Move to the next pending track/album

#### 2.9.6: Wire up the `tessitura review` command

```rust
Commands::Review,
```

Enters the TUI mode. Uses crossterm's raw mode for terminal control.

### Acceptance Criteria

- [ ] Album list view shows all albums awaiting review
- [ ] Track detail view shows proposed tags with conflict details
- [ ] Accept all approves entire album at once
- [ ] Track-by-track review allows individual approval/editing
- [ ] Keyboard navigation works (`j/k`, `Enter`, `Escape`, `q`)
- [ ] treadle review approval is called on accept
- [ ] TUI exits cleanly (terminal state restored)

---

## Milestone 2.10: Full Pipeline Wiring

### Goal

Wire scan → identify → enrich → harmonize → (review) as a complete treadle
Workflow. Test end-to-end with the test albums from Phase 1.

### Where

- `crates/tessitura-etl/src/pipeline.rs` — expand `build_pipeline()`
- `crates/tessitura-cli/src/main.rs` — wire new commands

### Dependencies

Milestones 2.6–2.9

### Steps

#### 2.10.1: Expand `build_pipeline()` to include enrich and harmonize

```rust
pub fn build_full_pipeline(
    music_dir: PathBuf,
    db_path: PathBuf,
    config: &Config,
) -> treadle::Result<Workflow> {
    let scan = ScanStage::new(music_dir, db_path.clone());
    let identify = IdentifyStage::new(
        config.acoustid_api_key.clone(), db_path.clone()
    );
    let enrich = EnrichStage::new(config, db_path.clone());
    let harmonize = HarmonizeStage::new(
        config.rules_path(), db_path.clone()
    );

    Workflow::builder()
        .stage("scan", scan)
        .stage("identify", identify)
        .stage("enrich", enrich)
        .stage("harmonize", harmonize)
        .dependency("identify", "scan")
        .dependency("enrich", "identify")
        .dependency("harmonize", "enrich")
        .build()
}
```

#### 2.10.2: Add new CLI commands to main.rs

Add `Enrich`, `Harmonize`, and `Review` to the `Commands` enum. Wire
them to their handler functions. Update `--help` text for all commands.

#### 2.10.3: Add `rules_path` to Config

```rust
/// Path to the mapping rules TOML file.
#[serde(default = "default_rules_path")]
pub rules_path: PathBuf,
```

Default: `{config_dir}/taxonomy.toml`

#### 2.10.4: End-to-end integration test

Create an integration test that mocks all external APIs and runs the full
pipeline from scan through harmonize. Verify database state at each stage.

### Acceptance Criteria

- [ ] Full 4-stage pipeline builds and runs
- [ ] Pipeline status shows progress through all stages
- [ ] Review pauses the pipeline (NeedsReview outcome)
- [ ] After review approval, pipeline state is updated
- [ ] `tessitura status` shows accurate stage progress including enrichment subtasks
- [ ] Integration test covers full pipeline with mocked APIs

---

## Milestone 2.11: Audio Decoding Integration

### Goal

Integrate `symphonia` for decoding various audio formats (FLAC, MP3, OGG, M4A, WAV)
to raw PCM samples. This provides the audio data needed for fingerprinting.

### Where

- `crates/tessitura-etl/src/audio/mod.rs` — audio module
- `crates/tessitura-etl/src/audio/decoder.rs` — symphonia integration
- `crates/tessitura-etl/Cargo.toml` — add symphonia dependency

### Dependencies

Phase 1 (scan stage exists)

### Steps

#### 2.11.1: Add symphonia dependency

Add to workspace `Cargo.toml`:

```toml
symphonia = { version = "0.5", features = ["aac", "alac", "flac", "mp3", "vorbis", "wav"] }
```

Add to `crates/tessitura-etl/Cargo.toml`:

```toml
symphonia = { workspace = true }
```

#### 2.11.2: Create audio decoder module

Create `crates/tessitura-etl/src/audio/decoder.rs`:

```rust
use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio as mono PCM samples at a specific sample rate.
#[derive(Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: f64,
}

/// Decode an audio file to mono PCM samples.
///
/// Resamples to target_sample_rate (typically 11025 or 16000 Hz for chromaprint).
/// Converts stereo to mono by averaging channels.
pub fn decode_audio(path: &Path, target_sample_rate: u32) -> Result<DecodedAudio> {
    // 1. Open the media source
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 2. Probe the format
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("Failed to probe audio format")?;

    let mut format = probed.format;

    // 3. Find the default audio track
    let track = format
        .default_track()
        .context("No default audio track found")?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    // 4. Create decoder
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    // 5. Decode all packets
    let mut sample_buf = None;
    let mut all_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e).context("Failed to read packet"),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                if sample_buf.is_none() {
                    let spec = *audio_buf.spec();
                    let duration = audio_buf.capacity() as u64;
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(ref mut buf) = sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    all_samples.extend_from_slice(buf.samples());
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e).context("Failed to decode packet"),
        }
    }

    // 6. Convert to mono if stereo (average channels)
    let channels = codec_params.channels.unwrap().count();
    let mono_samples = if channels > 1 {
        all_samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        all_samples
    };

    // 7. Resample if needed (simplified - for production, use a proper resampler)
    let source_rate = codec_params.sample_rate.unwrap_or(44100);
    let resampled = if source_rate != target_sample_rate {
        resample_simple(&mono_samples, source_rate, target_sample_rate)
    } else {
        mono_samples
    };

    let duration = resampled.len() as f64 / target_sample_rate as f64;

    Ok(DecodedAudio {
        samples: resampled,
        sample_rate: target_sample_rate,
        duration_secs: duration,
    })
}

/// Simple linear resampling (for production, consider using `rubato` or similar).
fn resample_simple(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        if idx + 1 < samples.len() {
            let frac = pos - idx as f64;
            let sample = samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32;
            output.push(sample);
        } else if idx < samples.len() {
            output.push(samples[idx]);
        }
    }

    output
}
```

#### 2.11.3: Add error handling for decode failures

Gracefully handle files that fail to decode:

- Log the error with file path
- Mark the item with `fingerprint = NULL`
- Continue processing other files

#### 2.11.4: Add unit tests with fixture audio files

Create short test audio files (1-2 second clips) in multiple formats:

- `tests/fixtures/test.flac`
- `tests/fixtures/test.mp3`
- `tests/fixtures/test.ogg`

Test that:

- Each format decodes successfully
- Mono and stereo files both work
- Sample rate conversion works
- Decode errors are handled gracefully

### Acceptance Criteria

- [ ] `symphonia` dependency added and compiling
- [ ] Audio decoder successfully decodes FLAC, MP3, OGG, WAV, M4A
- [ ] Stereo audio converted to mono correctly (channel averaging)
- [ ] Sample rate conversion to 11025 Hz works
- [ ] Decode errors handled gracefully (log + skip file)
- [ ] Unit tests pass with fixture audio files
- [ ] No panics on malformed audio files

---

## Milestone 2.12: Chromaprint Fingerprinting in Scan

### Goal

Integrate `rusty-chromaprint` to generate acoustic fingerprints during the scan
stage. Store fingerprints in the `Item.fingerprint` field for later use by
identification.

### Where

- `crates/tessitura-etl/src/audio/fingerprint.rs` — chromaprint integration
- `crates/tessitura-etl/src/scan.rs` — add fingerprinting to scan stage
- `crates/tessitura-etl/Cargo.toml` — add rusty-chromaprint dependency

### Dependencies

Milestone 2.11 (audio decoder exists)

### Steps

#### 2.12.1: Add rusty-chromaprint dependency

Add to workspace `Cargo.toml`:

```toml
rusty-chromaprint = "0.2"
```

Add to `crates/tessitura-etl/Cargo.toml`:

```toml
rusty-chromaprint = { workspace = true }
```

#### 2.12.2: Create fingerprint module

Create `crates/tessitura-etl/src/audio/fingerprint.rs`:

```rust
use anyhow::{Context, Result};
use rusty_chromaprint::{Configuration, Fingerprinter};
use std::path::Path;

use super::decoder::decode_audio;

/// Generate a chromaprint fingerprint for an audio file.
///
/// Returns the fingerprint string and duration in seconds.
pub fn generate_fingerprint(path: &Path) -> Result<(String, f64)> {
    // 1. Decode audio to mono PCM at 11025 Hz (chromaprint's preferred rate)
    let audio = decode_audio(path, 11025)
        .with_context(|| format!("Failed to decode audio: {}", path.display()))?;

    // 2. Create chromaprint fingerprinter
    let config = Configuration::preset_test1(); // or preset_test2, preset_test3
    let mut fpr = Fingerprinter::new(&config);

    // 3. Feed samples to fingerprinter
    fpr.start(audio.sample_rate)
        .context("Failed to start fingerprinter")?;

    fpr.feed(&audio.samples)
        .context("Failed to feed samples")?;

    fpr.finish().context("Failed to finish fingerprinter")?;

    // 4. Get the fingerprint
    let fingerprint = fpr
        .fingerprint()
        .context("Failed to get fingerprint")?
        .to_string();

    Ok((fingerprint, audio.duration_secs))
}
```

#### 2.12.3: Integrate into ScanStage

Modify `crates/tessitura-etl/src/scan.rs`:

```rust
// In the file scanning loop, after extracting tags:

// Generate fingerprint
let (fingerprint, duration) = match generate_fingerprint(&path) {
    Ok(result) => (Some(result.0), Some(result.1)),
    Err(e) => {
        log::warn!("Failed to fingerprint {}: {}", path.display(), e);
        (None, None)
    }
};

// Create item with fingerprint
let mut item = Item::new(path_str)
    .with_format(format)
    .with_duration(duration.unwrap_or(0.0));

if let Some(fp) = fingerprint {
    item.fingerprint = Some(fp);
}

// ... rest of item creation
```

#### 2.12.4: Update scan progress display

Show fingerprinting progress:

```
Scanning: /path/to/music/album/track.flac
  Tags extracted ✓
  Fingerprinting... ✓
```

#### 2.12.5: Performance consideration

Fingerprinting adds ~2-3 seconds per track. For your 7,415 tracks, this would
add ~5-6 hours to a full rescan. Consider:

- Adding a `--skip-fingerprint` flag for quick rescans
- Only fingerprinting new/changed files (already handled by mtime check)
- Showing estimated time remaining in progress display

### Acceptance Criteria

- [ ] `rusty-chromaprint` dependency added and compiling
- [ ] Fingerprints generated during scan for all supported formats
- [ ] Fingerprints stored in `Item.fingerprint` field
- [ ] Decode/fingerprint errors handled gracefully (logged, item created without fingerprint)
- [ ] Progress display shows fingerprinting status
- [ ] Scan performance acceptable (~2-3 sec per track)
- [ ] Tests verify fingerprints are deterministic (same file → same fingerprint)

---

## Milestone 2.13: AcoustID Lookup in Identify

### Goal

Implement actual AcoustID API integration in `AcoustIdClient`. Modify
`IdentifyStage` to use fingerprint lookups as the primary identification
method, falling back to metadata search when fingerprints are unavailable
or yield no results.

### Where

- `crates/tessitura-etl/src/acoustid.rs` — expand AcoustIdClient
- `crates/tessitura-etl/src/identify.rs` — update identification strategy

### Dependencies

Milestone 2.12 (fingerprints exist in database)

### Steps

#### 2.13.1: Implement AcoustID lookup method

Expand `crates/tessitura-etl/src/acoustid.rs`:

```rust
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AcoustIdClient {
    api_key: String,
    http: Client,
    rate_limiter: crate::enrich::resilience::RateLimiter,
}

impl AcoustIdClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: Client::new(),
            rate_limiter: crate::enrich::resilience::RateLimiter::new(3), // 3 req/sec
        }
    }

    /// Lookup a fingerprint via AcoustID API.
    ///
    /// Returns MusicBrainz recording IDs with confidence scores.
    pub async fn lookup(
        &self,
        fingerprint: &str,
        duration: f64,
    ) -> Result<Vec<AcoustIdMatch>> {
        self.rate_limiter.acquire().await;

        let response: AcoustIdResponse = self
            .http
            .post("https://api.acoustid.org/v2/lookup")
            .form(&[
                ("client", self.api_key.as_str()),
                ("meta", "recordings"),
                ("fingerprint", fingerprint),
                ("duration", &duration.to_string()),
            ])
            .send()
            .await
            .context("AcoustID API request failed")?
            .json()
            .await
            .context("Failed to parse AcoustID response")?;

        if response.status != "ok" {
            anyhow::bail!("AcoustID error: {}", response.status);
        }

        let mut matches = Vec::new();
        for result in response.results.unwrap_or_default() {
            for recording in result.recordings.unwrap_or_default() {
                matches.push(AcoustIdMatch {
                    recording_id: recording.id,
                    score: result.score,
                });
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        Ok(matches)
    }
}

#[derive(Debug, Clone)]
pub struct AcoustIdMatch {
    pub recording_id: String,
    pub score: f64,
}

#[derive(Debug, Deserialize)]
struct AcoustIdResponse {
    status: String,
    results: Option<Vec<AcoustIdResult>>,
}

#[derive(Debug, Deserialize)]
struct AcoustIdResult {
    score: f64,
    recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, Deserialize)]
struct AcoustIdRecording {
    id: String,
}
```

#### 2.13.2: Update IdentifyStage strategy

Modify `crates/tessitura-etl/src/identify.rs`:

```rust
// In the identification loop:

// Strategy:
// 1. If item has fingerprint, try AcoustID lookup first
// 2. If AcoustID succeeds (score > 0.8), use it
// 3. Otherwise, fall back to metadata search
// 4. If both fail, mark as unidentified

if let (Some(fingerprint), Some(duration)) = (&item.fingerprint, item.duration_secs) {
    log::debug!("Trying fingerprint lookup for {}", item.file_path);

    if let Some(acoustid) = &self.acoustid {
        match acoustid.lookup(fingerprint, duration).await {
            Ok(matches) if !matches.is_empty() => {
                let best = &matches[0];
                if best.score >= 0.8 {
                    log::debug!("AcoustID match: {} (score: {:.2})", best.recording_id, best.score);

                    // Fetch full recording details from MusicBrainz
                    let recording = self.musicbrainz
                        .get_recording(&best.recording_id)
                        .await?;

                    // Create FRBR entities...
                    // (existing code)

                    continue; // Skip metadata fallback
                }
            }
            Ok(_) => log::debug!("AcoustID returned no high-confidence matches"),
            Err(e) => log::warn!("AcoustID lookup failed: {}", e),
        }
    }
}

// Fallback to metadata search
log::debug!("Trying metadata search for {}", item.file_path);
// (existing metadata search code)
```

#### 2.13.3: Add `fingerprint_score` to Item

Store the AcoustID confidence score:

```rust
item.fingerprint_score = Some(best.score);
db.update_item(&item)?;
```

#### 2.13.4: Update progress display

```
Identifying: /path/to/track.flac
  Fingerprint lookup... ✓ (MusicBrainz ID: abc123, score: 0.95)

Identifying: /path/to/other.mp3
  Fingerprint lookup... ✗ (no match)
  Metadata search... ✓ (found via artist + title)
```

### Acceptance Criteria

- [ ] `AcoustIdClient.lookup()` successfully queries AcoustID API
- [ ] MusicBrainz recording IDs extracted from AcoustID response
- [ ] Fingerprint lookup attempted first for items with fingerprints
- [ ] Metadata search fallback works when fingerprint fails
- [ ] Confidence scores stored in `Item.fingerprint_score`
- [ ] Rate limiting at 3 req/sec enforced
- [ ] Progress display shows which method succeeded
- [ ] Tests with mocked AcoustID responses

---

## Milestone 2.14: Backfill Command for Existing Items

### Goal

Add `tessitura fingerprint` command to generate fingerprints for already-scanned
items. Critical for your existing 7,415 tracks that were scanned before
fingerprinting was implemented.

### Where

- `crates/tessitura-cli/src/commands/fingerprint.rs` — new command module
- `crates/tessitura-cli/src/main.rs` — add Fingerprint command

### Dependencies

Milestone 2.12 (fingerprint generation logic exists)

### Steps

#### 2.14.1: Create fingerprint command module

Create `crates/tessitura-cli/src/commands/fingerprint.rs`:

```rust
use anyhow::Result;
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::audio::fingerprint::generate_fingerprint;

pub fn run_fingerprint(db_path: PathBuf, force: bool) -> Result<()> {
    log::info!("Starting fingerprint backfill");

    let db = Database::open(&db_path)?;

    // Get items needing fingerprints
    let items = if force {
        db.list_all_items()?
    } else {
        db.list_items_without_fingerprints()?
    };

    if items.is_empty() {
        println!("No items need fingerprinting.");
        return Ok(());
    }

    println!(
        "Fingerprinting {} items... (this may take a while)",
        items.len()
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for (idx, item) in items.iter().enumerate() {
        print!("\r[{}/{}] {}", idx + 1, items.len(), item.file_path);
        std::io::Write::flush(&mut std::io::stdout())?;

        match generate_fingerprint(&PathBuf::from(&item.file_path)) {
            Ok((fingerprint, duration)) => {
                db.update_item_fingerprint(&item.id, Some(&fingerprint), Some(duration))?;
                success_count += 1;
            }
            Err(e) => {
                log::warn!("Failed to fingerprint {}: {}", item.file_path, e);
                error_count += 1;
            }
        }
    }

    println!("\n\n✓ Fingerprinting complete");
    println!("  Success: {}", success_count);
    println!("  Errors:  {}", error_count);

    Ok(())
}
```

#### 2.14.2: Add database query method

Add to `crates/tessitura-core/src/schema/db.rs`:

```rust
/// List all items without fingerprints.
pub fn list_items_without_fingerprints(&self) -> Result<Vec<Item>> {
    let mut stmt = self.conn.prepare(
        "SELECT * FROM items WHERE fingerprint IS NULL ORDER BY file_path"
    )?;
    let items = stmt.query_map([], |row| Item::from_row(row))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Update an item's fingerprint.
pub fn update_item_fingerprint(
    &self,
    item_id: &uuid::Uuid,
    fingerprint: Option<&str>,
    duration: Option<f64>,
) -> Result<()> {
    self.conn.execute(
        "UPDATE items SET fingerprint = ?1, duration_secs = ?2, updated_at = ?3
         WHERE id = ?4",
        rusqlite::params![
            fingerprint,
            duration,
            chrono::Utc::now().to_rfc3339(),
            item_id.to_string(),
        ],
    )?;
    Ok(())
}
```

#### 2.14.3: Add CLI command

Add to `crates/tessitura-cli/src/main.rs`:

```rust
Commands::Fingerprint {
    /// Re-fingerprint all items (not just missing ones)
    #[arg(long, default_value_t = false)]
    force: bool,
},

// In the match:
Commands::Fingerprint { force } => {
    commands::fingerprint::run_fingerprint(config.database_path, force)?;
}
```

#### 2.14.4: Add progress estimation

Show estimated time remaining based on average time per track:

```
Fingerprinting 7415 items... (estimated: 5h 12m)
[1234/7415] /path/to/track.flac  [=====>     ] 16.6%  (2h 45m remaining)
```

### Acceptance Criteria

- [ ] `tessitura fingerprint` command exists
- [ ] Fingerprints all items where `fingerprint IS NULL`
- [ ] `--force` flag re-fingerprints all items
- [ ] Progress bar shows current file and percentage
- [ ] Error handling: logs failures but continues
- [ ] Summary shows success/error counts
- [ ] Database updates persisted correctly
- [ ] Can be run incrementally (resume after interruption)

---

## Milestone 2.15: Unified Process Command

### Goal

Create a single `tessitura process` command that orchestrates the complete metadata processing pipeline from start to finish. This command runs all five required steps in sequence: Scan → Fingerprint → Identify → Enrich → Harmonize, stopping on failure with clear error reporting and supporting resume capability.

This addresses a critical usability gap: users currently must understand and manually execute five separate commands in the correct order, with proper error checking between each step. The `process` command provides a single entry point for the most common workflow.

### Where

- `crates/tessitura-cli/src/commands/process.rs` — new command implementation
- `crates/tessitura-cli/src/commands/mod.rs` — add process module
- `crates/tessitura-cli/src/main.rs` — add Process to Commands enum

### Dependencies

Milestone 2.10 (full pipeline wiring)

### Steps

#### 2.15.1: Create the process command module

Create `crates/tessitura-cli/src/commands/process.rs`:

```rust
use anyhow::{Context, Result};
use std::path::PathBuf;
use tessitura_core::schema::Database;
use tessitura_etl::{build_full_pipeline, Config, MusicFile};

/// Orchestrate the complete processing pipeline.
///
/// Steps:
/// 1. Scan - discover audio files and extract metadata
/// 2. Fingerprint - generate acoustic fingerprints
/// 3. Identify - match to MusicBrainz recordings
/// 4. Enrich - fetch metadata from external sources
/// 5. Harmonize - apply mapping rules and resolve conflicts
pub async fn run_process(
    music_dir: PathBuf,
    db_path: PathBuf,
    config: &Config,
    resume: bool,
) -> Result<()> {
    println!("\n🎵 Tessitura Full Processing Pipeline\n");
    println!("  Music directory: {}", music_dir.display());
    println!("  Database: {}", db_path.display());
    println!();

    // Track which steps have been completed
    let mut completed_steps: Vec<&str> = Vec::new();

    // Step 1: Scan (unless resuming and already complete)
    if !resume || should_run_scan(&db_path)? {
        println!("📁 Step 1/5: Scanning music directory...");
        super::run_scan(music_dir.clone(), db_path.clone())
            .await
            .context("Scan step failed")?;
        completed_steps.push("scan");
        println!("  ✓ Scan complete\n");
    } else {
        println!("📁 Step 1/5: Scan (skipped - already complete)\n");
    }

    // Step 2: Fingerprint (unless resuming and already complete)
    if !resume || should_run_fingerprint(&db_path)? {
        println!("🎵 Step 2/5: Generating acoustic fingerprints...");
        super::run_fingerprint(db_path.clone(), false)
            .await
            .context("Fingerprint step failed")?;
        completed_steps.push("fingerprint");
        println!("  ✓ Fingerprint complete\n");
    } else {
        println!("🎵 Step 2/5: Fingerprint (skipped - already complete)\n");
    }

    // Step 3: Identify (unless resuming and already complete)
    if !resume || should_run_identify(&db_path)? {
        println!("🔍 Step 3/5: Identifying recordings...");
        super::run_identify(db_path.clone(), config.acoustid_api_key.clone())
            .await
            .context("Identify step failed")?;
        completed_steps.push("identify");
        println!("  ✓ Identify complete\n");
    } else {
        println!("🔍 Step 3/5: Identify (skipped - already complete)\n");
    }

    // Steps 4-5: Build and run the full treadle pipeline
    // (enrich + harmonize are treadle stages)
    println!("📚 Step 4/5: Enriching metadata from external sources...");
    println!("⚖️  Step 5/5: Harmonizing and resolving conflicts...");

    let workflow = build_full_pipeline(music_dir.clone(), db_path.clone(), config)
        .context("Failed to build pipeline")?;

    // Create state store
    let parent = db_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Database path has no parent directory"))?;
    let state_path = parent.join("pipeline.db");
    let mut store = treadle::SqliteStateStore::open(&state_path)
        .await
        .context("Failed to open pipeline state store")?;

    // Create work item
    let work_item = MusicFile::new("process-job", music_dir);

    // Subscribe to events for progress display
    let mut events = workflow.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                treadle::WorkflowEvent::StageStarted { stage, .. } => {
                    println!("  ⏳ [{stage}] Starting...");
                }
                treadle::WorkflowEvent::StageCompleted { stage, .. } => {
                    println!("  ✓ [{stage}] Complete");
                }
                treadle::WorkflowEvent::StageFailed { stage, error, .. } => {
                    eprintln!("  ✗ [{stage}] FAILED: {error}");
                }
                treadle::WorkflowEvent::NeedsReview { stage, .. } => {
                    println!("  ⏸  [{stage}] Awaiting review");
                }
                _ => {}
            }
        }
    });

    // Execute the workflow
    workflow
        .advance(&work_item, &mut store)
        .await
        .context("Pipeline execution failed")?;

    println!("\n✓ Full processing pipeline complete!");
    println!("\nNext steps:");
    println!("  - Run 'tessitura review' to review and approve proposed metadata");
    println!("  - Run 'tessitura status' to see pipeline status");

    Ok(())
}

/// Check if scan should run (no items in database).
fn should_run_scan(db_path: &PathBuf) -> Result<bool> {
    if !db_path.exists() {
        return Ok(true);
    }
    let db = Database::open(db_path)?;
    let items = db.list_all_items()?;
    Ok(items.is_empty())
}

/// Check if fingerprint should run (items exist without fingerprints).
fn should_run_fingerprint(db_path: &PathBuf) -> Result<bool> {
    let db = Database::open(db_path)?;
    let items = db.list_items_without_fingerprints()?;
    Ok(!items.is_empty())
}

/// Check if identify should run (unidentified items exist).
fn should_run_identify(db_path: &PathBuf) -> Result<bool> {
    let db = Database::open(db_path)?;
    let items = db.list_unidentified_items()?;
    Ok(!items.is_empty())
}
```

#### 2.15.2: Add process command to CLI

Add to `crates/tessitura-cli/src/commands/mod.rs`:

```rust
pub mod process;
pub use process::run_process;
```

Add to `Commands` enum in `crates/tessitura-cli/src/main.rs`:

```rust
/// Process a music library through the full pipeline
#[command(
    long_about = "Orchestrates the complete metadata processing pipeline from start to finish:

  1. Scan - Discover audio files and extract embedded metadata
  2. Fingerprint - Generate acoustic fingerprints for identification
  3. Identify - Match recordings to MusicBrainz database
  4. Enrich - Fetch metadata from external sources (Wikidata, Last.fm, Discogs)
  5. Harmonize - Apply mapping rules and resolve conflicts

This command stops on failure and reports which step failed. Use --resume to
skip already-completed steps when restarting after a failure.

The process command is the recommended way to initialize a new music library
in Tessitura. After completion, use 'tessitura review' to approve the
proposed metadata changes.

Output:
  - Progress indicators for each step
  - Clear error messages indicating which step failed
  - Summary statistics at completion"
)]
Process {
    /// Path to the music directory
    path: PathBuf,

    /// Resume from last successful step (skip completed steps)
    #[arg(long, short, default_value_t = false)]
    resume: bool,
},
```

Wire in `main()`:

```rust
Commands::Process { path, resume } => {
    let config = Config::load()?;
    let db_path = cli.db.unwrap_or_else(default_db_path);
    commands::run_process(path, db_path, &config, resume).await?;
}
```

#### 2.15.3: Update status command to show pipeline progress

Enhance `show_status()` to display which steps have been completed:

```rust
pub fn show_status(db_path: PathBuf, _filter: Option<String>) -> Result<()> {
    let db = Database::open(&db_path)?;

    // Get statistics for each pipeline step
    let total_items = db.list_all_items()?.len();
    let items_without_fingerprints = db.list_items_without_fingerprints()?.len();
    let unidentified_items = db.list_unidentified_items()?.len();
    let identified_items = db.list_identified_items()?.len();

    println!("\n📊 Tessitura Status\n");
    println!("  Database: {}", db_path.display());
    println!();
    println!("Pipeline Progress:");
    println!("  ✓ Scan:        {} items", total_items);
    println!(
        "  {} Fingerprint: {} items {}",
        if items_without_fingerprints == 0 { "✓" } else { "⏳" },
        total_items - items_without_fingerprints,
        if items_without_fingerprints > 0 {
            format!("({} pending)", items_without_fingerprints)
        } else {
            String::new()
        }
    );
    println!(
        "  {} Identify:    {} items {}",
        if unidentified_items == 0 { "✓" } else { "⏳" },
        identified_items,
        if unidentified_items > 0 {
            format!("({} pending)", unidentified_items)
        } else {
            String::new()
        }
    );
    // TODO: Add enrich and harmonize status when those stages track completion

    if unidentified_items > 0 {
        println!("\nNext step: Run 'tessitura process --resume' to continue processing");
    } else if identified_items > 0 {
        println!("\nNext step: Run 'tessitura review' to review proposed metadata");
    }

    Ok(())
}
```

#### 2.15.4: Add error context and recovery guidance

Ensure each step failure includes:

- Which step failed
- The error details
- How to resume (use `--resume` flag)
- How to run just that step individually if needed

#### 2.15.5: Integration test

Create `tests/integration/test_process_command.rs`:

```rust
#[tokio::test]
async fn test_process_command_full_pipeline() {
    // Setup: temp directory with test audio files
    // Mock external APIs (MusicBrainz, Wikidata, etc.)
    // Run: process command with test music dir
    // Verify: each step completes, database has expected data
    // Test: --resume flag skips already-completed steps
}

#[tokio::test]
async fn test_process_command_failure_recovery() {
    // Setup: temp directory, mock API that fails on 3rd call
    // Run: process command (should fail at identify)
    // Verify: error message indicates identify step failed
    // Run: process command with --resume
    // Verify: skips scan and fingerprint, retries identify
}
```

### Acceptance Criteria

- [ ] `tessitura process <dir>` runs all 5 steps in sequence
- [ ] Process stops on first failure with clear error message indicating which step failed
- [ ] `--resume` flag skips already-completed steps
- [ ] Status command shows progress through pipeline steps
- [ ] Each step's output clearly indicates success/failure
- [ ] Failed steps can be retried without re-running successful steps
- [ ] Documentation in `--help` explains the pipeline flow
- [ ] Integration tests cover happy path and failure recovery
- [ ] Process command properly handles:
  - Empty directory (no audio files)
  - Already-processed library (all steps complete)
  - Partial completion (some steps done, resume works)
  - Missing API keys (clear error, suggests configuration)

### Notes

- Fingerprint is intentionally kept separate from the treadle workflow for now, as it's a batch operation that doesn't fit the treadle stage model
- The `--resume` flag uses simple database queries to check completion rather than complex state tracking
- Future enhancement: add `--from-step` flag to start from a specific step
- Future enhancement: add `--to-step` flag to stop at a specific step (e.g., process up to identify only)

---

## Milestone 2.16: Mapping Rules Iteration

### Goal

Process test albums through the full pipeline repeatedly, refining mapping
rules until harmonization output is satisfactory for each genre. Document
the rule patterns that emerge.

### Where

- `config/taxonomy.toml` — refine rules
- `docs/mapping-patterns.md` — documentation

### Dependencies

Milestone 2.10

### Steps

#### 2.16.1: Process classical test albums

Run the Bartók String Quartets through the pipeline. Verify:

- Works are correctly distinguished (6 separate quartets)
- Form is "String quartet" (not "Classical")
- Period is "20th Century"
- Key is extracted from Wikidata (A minor for SQ No.1, etc.)
- Instrumentation is "2vn, va, vc"
- Catalog numbers (Sz. and BB.) are captured

Refine genre_rules and period_rules as needed.

#### 2.16.2: Process jazz test albums

Run Kind of Blue. Verify jazz-specific handling, artist roles.

#### 2.16.3: Process electronic test albums

Run Boards of Canada. Verify electronic subgenre handling with Last.fm tags.

#### 2.16.4: Process prog test albums

Run King Crimson. Verify progressive rock classification.

#### 2.16.5: Document emerged patterns

Write `docs/mapping-patterns.md` documenting:

- Which rules worked well out of the box
- Which rules needed iteration
- Common failure modes (wrong genre/form distinction, period inference gaps)
- Recommendations for users writing custom rules

### Acceptance Criteria

- [ ] At least 3 test albums processed through full pipeline
- [ ] Harmonization output is satisfactory for each genre
- [ ] Mapping rules handle genre/form distinction correctly
- [ ] Conflict resolution produces sensible results
- [ ] Documentation captures patterns and lessons learned

---

## Appendix A: New Dependency Checklist

| Crate | Purpose | Used By | Version |
|---|---|---|---|
| `failsafe` | Circuit breaker per enrichment source | tessitura-etl | 1 |
| `ratatui` | Terminal UI for review workflow | tessitura-cli | 0.29 |
| `crossterm` | Cross-platform terminal backend | tessitura-cli | 0.28 |
| `urlencoding` | URL-encode API query parameters | tessitura-etl | 2 |
| `toml` | Parse mapping rules TOML files | tessitura-core | 0.8 |
| `symphonia` | Audio decoding (FLAC, MP3, OGG, M4A, WAV) | tessitura-etl | 0.5 |
| `rusty-chromaprint` | Audio fingerprinting (pure Rust) | tessitura-etl | 0.2 |

Already in workspace: `backon`, `reqwest`, `serde_json`, `toml_edit`.

## Appendix B: External API Quick Reference

| API | Rate Limit | Auth | Key Endpoints |
|---|---|---|---|
| MusicBrainz | 1 req/sec | User-Agent header | `/ws/2/recording/{id}`, `/ws/2/work/{id}`, `/ws/2/release/{id}` |
| Wikidata | ~5 req/sec | None | SPARQL endpoint, `Special:EntityData/{QID}.json` |
| Last.fm | 5 req/sec | API key | `track.getTopTags`, `artist.getTopTags` |
| Discogs | 60/min (unauth), 240/min (auth) | Optional token | `/releases/{id}`, `/database/search` |

## Appendix C: Mapping Rules TOML Format

See `config/taxonomy.toml`. Key sections:

- `[source_priority]` — source_name → priority (higher wins)
- `[[genre_rules]]` — match_any, match_source, output_genre, output_form, output_lcgft_label
- `[[period_rules]]` — match_composer, output_period, year_range
- `[[instrument_rules]]` — match_any, output_instruments, output_lcmpt_labels

Example:

```toml
[source_priority]
embedded_tag = 1
lastfm = 2
discogs = 3
musicbrainz = 5
wikidata = 6
lcgft = 8
user = 10

[[genre_rules]]
name = "classical-20th-century"
match_any = ["modern classical", "20th century classical", "20th century"]
output_genre = "Classical > 20th Century"
output_lcgft_label = "Art music"
confidence = 0.9

[[genre_rules]]
name = "string-quartet-form"
match_any = ["string quartets", "string quartet"]
match_source = ["lcgft", "musicbrainz", "wikidata"]
output_form = "String quartet"
output_lcgft_label = "String quartets"
confidence = 1.0

[[period_rules]]
name = "20th-century"
match_composer = ["Bartók", "Stravinsky", "Schoenberg", "Prokofiev", "Shostakovich"]
output_period = "20th Century"
year_range = [1900, 1999]
```

## Appendix D: New File Inventory

### tessitura-core

```
src/taxonomy/rules.rs          — MappingRules, GenreRule, PeriodRule, InstrumentRule
src/taxonomy/genre.rs          — ADD LcgftTerm type
src/taxonomy/instrumentation.rs — ADD LcmptTerm type
src/schema/migrations.rs       — ADD migration 002 (vocabulary tables)
src/schema/db.rs               — Complete todo!() stubs, add vocabulary CRUD
```

### tessitura-etl

```
src/error.rs                   — EnrichError, EnrichResult
src/enrich/mod.rs              — Module declarations
src/enrich/resilience.rs       — RateLimiter, shared resilience utilities
src/enrich/musicbrainz.rs      — MusicBrainzEnricher
src/enrich/wikidata.rs         — WikidataClient, WikidataEnricher
src/enrich/lastfm.rs           — LastFmClient, LastFmEnricher
src/enrich/discogs.rs          — DiscogsClient, DiscogsEnricher
src/enrich/lcgft.rs            — load_lcgft(), load_lcmpt()
src/enrich/stage.rs            — EnrichStage (fan-out treadle Stage)
src/harmonize.rs               — HarmonizeStage, conflict resolution
src/audio/mod.rs               — Audio module
src/audio/decoder.rs           — Symphonia audio decoding
src/audio/fingerprint.rs       — Chromaprint fingerprinting
src/acoustid.rs                — EXPAND: add lookup() method
src/identify.rs                — UPDATE: fingerprint-first strategy
```

### tessitura-cli

```
src/tui/mod.rs                 — App state, event loop, run_tui()
src/tui/album_list.rs          — Album list view renderer
src/tui/track_detail.rs        — Track detail view renderer
src/commands/enrich.rs         — run_enrich()
src/commands/harmonize.rs      — run_harmonize()
src/commands/review.rs         — run_review()
src/commands/fingerprint.rs    — run_fingerprint() (backfill command)
```

### Config / Data

```
config/taxonomy.toml           — Default mapping rules
config/lcgft-snapshot.json     — LCGFT vocabulary snapshot
config/lcmpt-snapshot.json     — LCMPT vocabulary snapshot
```

## Verification

### Build and Test

```bash
cargo build --workspace
cargo test --workspace
make check  # build + lint + test
```

### Manual Testing (per milestone)

1. **2.0**: `cargo test -p tessitura-core` — verify CRUD round-trips
2. **2.1-2.5**: Mock API tests for each enrichment source
3. **2.6**: `tessitura enrich` with mocked APIs → assertions in DB
4. **2.7**: Unit tests for rule matching against sample assertions
5. **2.8**: `tessitura harmonize` → proposed tags stored, NeedsReview status
6. **2.9**: `tessitura review` → TUI launches, navigation works, accept updates state
7. **2.10**: Full pipeline end-to-end with test albums
8. **2.11**: Audio decode tests with fixture files (FLAC, MP3, OGG)
9. **2.12**: Fingerprint generation in scan → stored in database
10. **2.13**: AcoustID lookup with mocked API → correct MusicBrainz IDs
11. **2.14**: `tessitura fingerprint` backfills existing items
12. **2.16**: Process real albums, refine rules, verify output quality

### Environment Variables for Testing

```bash
export ACOUSTID_API_KEY="..."
export TESS_LASTFM_API_KEY="..."
export TESS_DISCOGS_TOKEN="..."
```
