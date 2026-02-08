# Phase 2 Implementation Plan: Enrichment + Harmonization + Review

## Context

Phase 1 is complete: the FRBR data model, SQLite schema, scan stage, identify stage, and CLI are working with 43 tests passing. The project plan (v1.1) defines Phase 2 as milestones 2.1–2.11 covering: fan-out enrichment from 5 external sources (MusicBrainz, Wikidata, Last.fm, LCGFT/LCMPT, Discogs), a mapping rules engine for harmonization, and a ratatui-based review TUI.

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
12. [Milestone 2.11: Mapping Rules Iteration](#milestone-211-mapping-rules-iteration)
13. [Appendix A: New Dependency Checklist](#appendix-a-new-dependency-checklist)
14. [Appendix B: External API Quick Reference](#appendix-b-external-api-quick-reference)
15. [Appendix C: Mapping Rules TOML Format](#appendix-c-mapping-rules-toml-format)
16. [Appendix D: New File Inventory](#appendix-d-new-file-inventory)

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

## Milestone 2.11: Mapping Rules Iteration

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

#### 2.11.1: Process classical test albums

Run the Bartók String Quartets through the pipeline. Verify:

- Works are correctly distinguished (6 separate quartets)
- Form is "String quartet" (not "Classical")
- Period is "20th Century"
- Key is extracted from Wikidata (A minor for SQ No.1, etc.)
- Instrumentation is "2vn, va, vc"
- Catalog numbers (Sz. and BB.) are captured

Refine genre_rules and period_rules as needed.

#### 2.11.2: Process jazz test albums

Run Kind of Blue. Verify jazz-specific handling, artist roles.

#### 2.11.3: Process electronic test albums

Run Boards of Canada. Verify electronic subgenre handling with Last.fm tags.

#### 2.11.4: Process prog test albums

Run King Crimson. Verify progressive rock classification.

#### 2.11.5: Document emerged patterns

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
```

### tessitura-cli
```
src/tui/mod.rs                 — App state, event loop, run_tui()
src/tui/album_list.rs          — Album list view renderer
src/tui/track_detail.rs        — Track detail view renderer
src/commands/enrich.rs         — run_enrich()
src/commands/harmonize.rs      — run_harmonize()
src/commands/review.rs         — run_review()
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
8. **2.11**: Process real albums, refine rules, verify output quality

### Environment Variables for Testing
```bash
export ACOUSTID_API_KEY="..."
export TESS_LASTFM_API_KEY="..."
export TESS_DISCOGS_TOKEN="..."
```
