# Phase 1 Implementation Plan: Core Data Model + Scan + Identify

**Target:** Establish the FRBR-rooted schema, scan a music directory, identify
recordings via AcoustID/MusicBrainz, and process test albums end-to-end.

**Deliverable:** `tessitura scan /path/to/music` and `tessitura identify`
working against real audio files, with results stored in SQLite.

---

## Table of Contents

1. [Milestone 1.1: Workspace Scaffold](#milestone-11-workspace-scaffold)
2. [Milestone 1.2: FRBR Model Types](#milestone-12-frbr-model-types)
3. [Milestone 1.3: SQLite Schema](#milestone-13-sqlite-schema)
4. [Milestone 1.4: Taxonomy Stubs](#milestone-14-taxonomy-stubs)
5. [Milestone 1.5: Scan Stage](#milestone-15-scan-stage)
6. [Milestone 1.6: Identify Stage](#milestone-16-identify-stage)
7. [Milestone 1.7: Pipeline Wiring](#milestone-17-pipeline-wiring)
8. [Milestone 1.8: Test Album Validation](#milestone-18-test-album-validation)

---

## Milestone 1.1: Workspace Scaffold

### Goal

Convert the placeholder crate into a Cargo workspace with all five crate stubs,
CI configuration, and formatting/linting config.

### Steps

#### 1.1.1: Convert root Cargo.toml to workspace manifest

Replace the current root `Cargo.toml` (which defines a single `[package]`)
with a workspace manifest. The publishable package metadata moves into
`crates/tessitura-core/Cargo.toml`.

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "3"
members = [
    "crates/tessitura-core",
    "crates/tessitura-etl",
    "crates/tessitura-graph",
    "crates/tessitura-search",
    "crates/tessitura-cli",
]

[workspace.package]
edition = "2024"
license = "Apache-2.0 OR MIT"
repository = "https://github.com/oxur/tessitura"
homepage = "https://github.com/oxur/tessitura"

[workspace.dependencies]
# Core
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive", "cargo", "wrap_help"] }
uuid = { version = "1", features = ["v4", "serde"] }

# Domain
treadle = "0.2"
rusqlite = { version = "0.33", features = ["bundled"] }
lofty = "0.22"
petgraph = "0.7"
reqwest = { version = "0.12", features = ["json"] }

# Resilience
backon = "1"

# Internal crates
tessitura-core = { path = "crates/tessitura-core" }
tessitura-etl = { path = "crates/tessitura-etl" }
tessitura-graph = { path = "crates/tessitura-graph" }
tessitura-search = { path = "crates/tessitura-search" }

# Dev
tempfile = "3"

[workspace.lints.rust]
unsafe_code = "deny"
missing_debug_implementations = "warn"
unused_lifetimes = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
cargo = { level = "warn", priority = -1 }
# Allow these common pedantic triggers:
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
```

**Note on dependency versions:** The versions above are reasonable starting
points. Before implementation, verify current versions on crates.io and
update as needed. In particular:

- `treadle`: confirm latest version (0.2.x) and that the API matches what is
  described in this document
- `lofty`: confirm latest stable version
- `rusqlite`: confirm latest version and that `bundled` feature is still the
  recommended approach

#### 1.1.2: Create crate directories and stub Cargo.toml files

Create the five crate directories under `crates/`. Each gets a `Cargo.toml`
and a `src/lib.rs` (or `src/main.rs` for the CLI crate).

**Delete `src/lib.rs` from the workspace root** — it was only a placeholder.

**crates/tessitura-core/Cargo.toml:**

```toml
[package]
name = "tessitura-core"
version = "0.1.0"
description = "Core domain model and schema for tessitura"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
rusqlite = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

**crates/tessitura-etl/Cargo.toml:**

```toml
[package]
name = "tessitura-etl"
version = "0.1.0"
description = "ETL pipeline stages for tessitura"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
tessitura-core = { workspace = true }
treadle = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
lofty = { workspace = true }
reqwest = { workspace = true }
backon = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

**crates/tessitura-graph/Cargo.toml:**

```toml
[package]
name = "tessitura-graph"
version = "0.1.0"
description = "Music knowledge graph for tessitura"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
tessitura-core = { workspace = true }
petgraph = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**crates/tessitura-search/Cargo.toml:**

```toml
[package]
name = "tessitura-search"
version = "0.1.0"
description = "Vector search for tessitura"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
tessitura-core = { workspace = true }
thiserror = { workspace = true }

[lints]
workspace = true
```

**crates/tessitura-cli/Cargo.toml:**

```toml
[package]
name = "tessitura"
version = "0.1.0"
description = "A musicological library cataloging tool for serious musicians"
edition.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "../../README.md"
keywords = ["music", "metadata", "cataloging", "musicology", "audio"]
categories = ["command-line-utilities", "multimedia::audio"]

[[bin]]
name = "tessitura"
path = "src/main.rs"

[dependencies]
tessitura-core = { workspace = true }
tessitura-etl = { workspace = true }
tessitura-graph = { workspace = true }
tessitura-search = { workspace = true }
clap = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[lints]
workspace = true
```

**Important:** The CLI crate's `[package]` name is `"tessitura"` — this is
the crate that gets published to crates.io and produces the `tessitura`
binary. All other crates use their prefixed names.

#### 1.1.3: Create stub source files

Each library crate gets a `src/lib.rs` with a crate-level doc comment:

**crates/tessitura-core/src/lib.rs:**

```rust
//! Core domain model for tessitura.
//!
//! This crate defines the FRBR-rooted data model (Work, Expression,
//! Manifestation, Item), the SQLite schema, taxonomy types, and
//! provenance tracking.
```

**crates/tessitura-etl/src/lib.rs:**

```rust
//! ETL pipeline stages for tessitura.
//!
//! Implements the scan, identify, enrich, harmonize, index, and export
//! stages as treadle `Stage` implementations.
```

**crates/tessitura-graph/src/lib.rs:**

```rust
//! Music knowledge graph for tessitura.
//!
//! Builds and queries a petgraph-based graph of works, artists, genres,
//! forms, periods, and their relationships.
```

**crates/tessitura-search/src/lib.rs:**

```rust
//! Vector search for tessitura.
//!
//! Manages LanceDB vector indices for similarity and fuzzy search
//! across the music catalog.
```

**crates/tessitura-cli/src/main.rs:**

```rust
use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "tessitura", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Scan a music directory for audio files
    Scan {
        /// Path to the music directory
        path: std::path::PathBuf,
    },
    /// Identify recordings via AcoustID/MusicBrainz
    Identify,
    /// Show pipeline status
    Status {
        /// Optional filter (album name, artist, etc.)
        filter: Option<String>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env(),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path } => {
            println!("Scanning: {}", path.display());
            todo!("Implement in milestone 1.7")
        }
        Commands::Identify => {
            todo!("Implement in milestone 1.7")
        }
        Commands::Status { filter } => {
            let _ = filter;
            todo!("Implement in milestone 1.7")
        }
    }
}
```

#### 1.1.4: Create rustfmt.toml

```toml
# rustfmt.toml
edition = "2024"
max_width = 100
use_field_init_shorthand = true
```

#### 1.1.5: Create clippy.toml (if needed)

Generally the workspace-level `[workspace.lints.clippy]` in `Cargo.toml` is
sufficient. Only create `clippy.toml` if you need threshold overrides:

```toml
# clippy.toml
too-many-arguments-threshold = 8
type-complexity-threshold = 300
```

#### 1.1.6: Create GitHub Actions CI

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - name: Format check
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --all-features --workspace -- -D warnings
      - name: Build
        run: cargo build --workspace
      - name: Test
        run: cargo test --all-features --workspace
```

#### 1.1.7: Verify

Run `cargo build --workspace` and `cargo test --workspace` — both should
succeed with the stub crates. Run `cargo clippy --workspace` and
`cargo fmt --check` to confirm linting is clean.

### Acceptance Criteria

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` succeeds
- [ ] `cargo clippy --workspace -- -D warnings` is clean
- [ ] `cargo fmt --all -- --check` is clean
- [ ] `cargo run -- --help` prints help text with scan/identify/status subcommands
- [ ] Root `src/lib.rs` is deleted (no longer a standalone crate)
- [ ] Five crate directories exist under `crates/`

---

## Milestone 1.2: FRBR Model Types

### Goal

Define the core domain types representing the FRBR hierarchy: Work,
Expression (performance/recording), Manifestation (release), and Item
(file). These are the foundational types used throughout the system.

### Where

All types go in `crates/tessitura-core/src/model/`.

### Steps

#### 1.2.1: Create the model module structure

```
crates/tessitura-core/src/
├── lib.rs
├── error.rs
├── model/
│   ├── mod.rs
│   ├── work.rs
│   ├── expression.rs
│   ├── manifestation.rs
│   ├── item.rs
│   ├── artist.rs
│   └── ids.rs
└── provenance.rs
```

Update `lib.rs`:

```rust
//! Core domain model for tessitura.

pub mod error;
pub mod model;
pub mod provenance;

pub use error::{Error, Result};
```

#### 1.2.2: Define ID newtypes in `ids.rs`

Use newtype wrappers around UUIDs for type safety. Each FRBR entity gets
its own ID type so you cannot accidentally pass a `WorkId` where an
`ExpressionId` is expected.

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<Uuid> for $name {
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }
    };
}

define_id!(WorkId, "Unique identifier for a musical work.");
define_id!(ExpressionId, "Unique identifier for a performance/recording.");
define_id!(ManifestationId, "Unique identifier for a release.");
define_id!(ItemId, "Unique identifier for a physical/digital file.");
define_id!(ArtistId, "Unique identifier for an artist.");
```

#### 1.2.3: Define the Work type in `work.rs`

A Work is a distinct intellectual/artistic creation: Beethoven's Symphony
No. 5, Bartók's String Quartet No. 4, etc.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::model::ids::WorkId;

/// A distinct musical work (composition).
///
/// Corresponds to the FRBR "Work" level. A Work is an abstract
/// intellectual creation, independent of any specific performance
/// or recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work {
    pub id: WorkId,
    pub title: String,
    pub composer: Option<String>,

    /// MusicBrainz work ID, if identified.
    pub musicbrainz_id: Option<String>,

    /// Catalog number (BWV, K., Sz., BB., Op., etc.).
    pub catalog_number: Option<String>,

    /// Musical key (e.g., "A minor", "D major").
    pub key: Option<String>,

    /// Year or approximate date of composition.
    pub composed_year: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Work {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: WorkId::new(),
            title: title.into(),
            composer: None,
            musicbrainz_id: None,
            catalog_number: None,
            key: None,
            composed_year: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_composer(mut self, composer: impl Into<String>) -> Self {
        self.composer = Some(composer.into());
        self
    }

    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    pub fn with_catalog_number(mut self, catalog: impl Into<String>) -> Self {
        self.catalog_number = Some(catalog.into());
        self
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_composed_year(mut self, year: i32) -> Self {
        self.composed_year = Some(year);
        self
    }
}
```

#### 1.2.4: Define the Expression type in `expression.rs`

An Expression is a specific realization of a Work — a particular
performance or recording. Karajan/BPO 1962 is a different Expression
of Beethoven's 5th than Bernstein/NYP 1958.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::model::ids::{ExpressionId, WorkId, ArtistId};

/// A specific performance or recording of a Work.
///
/// Corresponds to the FRBR "Expression" level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    pub id: ExpressionId,
    pub work_id: WorkId,
    pub title: Option<String>,

    /// MusicBrainz recording ID.
    pub musicbrainz_id: Option<String>,

    /// Primary performer IDs (soloists, ensembles).
    pub performer_ids: Vec<ArtistId>,

    /// Conductor, if applicable.
    pub conductor_id: Option<ArtistId>,

    /// Recording year.
    pub recorded_year: Option<i32>,

    /// Duration in seconds.
    pub duration_secs: Option<f64>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Expression {
    pub fn new(work_id: WorkId) -> Self {
        let now = Utc::now();
        Self {
            id: ExpressionId::new(),
            work_id,
            title: None,
            musicbrainz_id: None,
            performer_ids: Vec::new(),
            conductor_id: None,
            recorded_year: None,
            duration_secs: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    pub fn with_performer(mut self, performer_id: ArtistId) -> Self {
        self.performer_ids.push(performer_id);
        self
    }

    pub fn with_conductor(mut self, conductor_id: ArtistId) -> Self {
        self.conductor_id = Some(conductor_id);
        self
    }

    pub fn with_duration(mut self, secs: f64) -> Self {
        self.duration_secs = Some(secs);
        self
    }
}
```

#### 1.2.5: Define the Manifestation type in `manifestation.rs`

A Manifestation is a specific release — a particular CD, LP, or digital
release with a label, catalog number, and track listing.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::model::ids::ManifestationId;

/// A specific release (CD, LP, digital) of one or more recordings.
///
/// Corresponds to the FRBR "Manifestation" level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifestation {
    pub id: ManifestationId,
    pub title: String,

    /// MusicBrainz release ID.
    pub musicbrainz_id: Option<String>,

    /// Record label.
    pub label: Option<String>,

    /// Label catalog number (e.g., "455 297-2").
    pub catalog_number: Option<String>,

    /// Release year.
    pub release_year: Option<i32>,

    /// Number of tracks/discs.
    pub track_count: Option<u32>,
    pub disc_count: Option<u32>,

    /// Media format: "CD", "LP", "Digital", etc.
    pub format: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Manifestation {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ManifestationId::new(),
            title: title.into(),
            musicbrainz_id: None,
            label: None,
            catalog_number: None,
            release_year: None,
            track_count: None,
            disc_count: None,
            format: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_release_year(mut self, year: i32) -> Self {
        self.release_year = Some(year);
        self
    }
}
```

#### 1.2.6: Define the Item type in `item.rs`

An Item is a specific physical or digital copy — in our case, an audio
file on disk. This is the entry point from the filesystem into the data
model.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::model::ids::{ItemId, ExpressionId, ManifestationId};

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

    /// AcoustID fingerprint (computed during Scan stage).
    pub fingerprint: Option<String>,

    /// AcoustID score (confidence of the fingerprint match, 0.0-1.0).
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
    pub fn new(file_path: PathBuf, format: AudioFormat, file_size: u64, file_mtime: DateTime<Utc>) -> Self {
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
    pub fn is_identified(&self) -> bool {
        self.expression_id.is_some()
    }
}
```

#### 1.2.7: Define the Artist type in `artist.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::model::ids::ArtistId;

/// The role an artist plays in a musical context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtistRole {
    Composer,
    Performer,
    Conductor,
    Ensemble,
    Producer,
    Other,
}

/// A musical artist (person or group).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub sort_name: Option<String>,

    /// MusicBrainz artist ID.
    pub musicbrainz_id: Option<String>,

    /// Primary role(s) this artist is known for.
    pub roles: Vec<ArtistRole>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artist {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ArtistId::new(),
            name: name.into(),
            sort_name: None,
            musicbrainz_id: None,
            roles: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_role(mut self, role: ArtistRole) -> Self {
        self.roles.push(role);
        self
    }

    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }
}
```

#### 1.2.8: Wire up `model/mod.rs`

```rust
pub mod artist;
pub mod expression;
pub mod ids;
pub mod item;
pub mod manifestation;
pub mod work;

pub use artist::{Artist, ArtistRole};
pub use expression::Expression;
pub use ids::{ArtistId, ExpressionId, ItemId, ManifestationId, WorkId};
pub use item::{AudioFormat, Item};
pub use manifestation::Manifestation;
pub use work::Work;
```

#### 1.2.9: Define the error module in `error.rs`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("not found: {entity} with id {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

#### 1.2.10: Define provenance tracking in `provenance.rs`

Every metadata assertion has provenance: which source provided it,
when, and with what confidence.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The source of a metadata assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    /// Extracted from embedded audio file tags.
    EmbeddedTag,
    /// AcoustID fingerprint matching.
    AcoustId,
    /// MusicBrainz database.
    MusicBrainz,
    /// Wikidata.
    Wikidata,
    /// Last.fm folksonomy tags.
    LastFm,
    /// Library of Congress Genre/Form Terms.
    Lcgft,
    /// Library of Congress Medium of Performance Thesaurus.
    Lcmpt,
    /// Discogs database.
    Discogs,
    /// Manual entry by the user.
    User,
}

/// A metadata assertion with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assertion {
    /// The entity this assertion is about (work, expression, etc.).
    pub entity_id: String,

    /// The field being asserted (e.g., "genre", "key", "form").
    pub field: String,

    /// The asserted value.
    pub value: serde_json::Value,

    /// Where this assertion came from.
    pub source: Source,

    /// Confidence score (0.0 to 1.0), if applicable.
    pub confidence: Option<f64>,

    /// When this assertion was fetched/created.
    pub fetched_at: DateTime<Utc>,
}

impl Assertion {
    pub fn new(
        entity_id: impl Into<String>,
        field: impl Into<String>,
        value: serde_json::Value,
        source: Source,
    ) -> Self {
        Self {
            entity_id: entity_id.into(),
            field: field.into(),
            value,
            source,
            confidence: None,
            fetched_at: Utc::now(),
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }
}
```

### Acceptance Criteria

- [ ] All FRBR types compile with `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`
- [ ] ID newtypes provide type safety (cannot mix WorkId with ExpressionId)
- [ ] Builder-style constructors work: `Work::new("title").with_composer("Bartók")`
- [ ] Provenance types are defined and usable
- [ ] `cargo test --workspace` still passes
- [ ] Unit tests exist for ID generation, AudioFormat detection, builder patterns

---

## Milestone 1.3: SQLite Schema

### Goal

Design and implement the SQLite schema with a migration system. Tables
for works, expressions, manifestations, items, artists, relationships,
and provenance assertions.

### Where

`crates/tessitura-core/src/schema/`

### Steps

#### 1.3.1: Create the schema module

```
crates/tessitura-core/src/schema/
├── mod.rs
├── migrations.rs
└── db.rs
```

#### 1.3.2: Define the migration system in `migrations.rs`

Use a simple versioned migration approach. Each migration is a SQL string
with a version number. The migrations table tracks which have been applied.

```rust
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: MIGRATION_001,
    },
];
```

#### 1.3.3: Define the initial schema (MIGRATION_001)

This is the SQL for the initial schema. Key design points:

- UUIDs stored as TEXT (SQLite has no native UUID type)
- Timestamps stored as TEXT in ISO 8601 format (SQLite convention)
- Foreign keys enabled via `PRAGMA foreign_keys = ON`
- Many-to-many relationships use junction tables

```sql
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
```

#### 1.3.4: Implement the Database wrapper in `db.rs`

Create a `Database` struct that owns a `rusqlite::Connection`, applies
migrations on open, and provides typed CRUD methods for each entity.

```rust
use rusqlite::Connection;
use std::path::Path;
use crate::error::Result;

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
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn apply_migrations(&self) -> Result<()> {
        // ... apply unapplied migrations from MIGRATIONS
    }
}
```

Then implement CRUD methods for each entity type. Group them in impl
blocks or separate modules as they grow. At minimum for Phase 1:

**Items (most important — used by Scan and Identify stages):**

- `insert_item(&self, item: &Item) -> Result<()>`
- `update_item(&self, item: &Item) -> Result<()>`
- `get_item_by_id(&self, id: &ItemId) -> Result<Option<Item>>`
- `get_item_by_path(&self, path: &Path) -> Result<Option<Item>>`
- `list_items(&self) -> Result<Vec<Item>>`
- `list_unidentified_items(&self) -> Result<Vec<Item>>`
- `delete_item(&self, id: &ItemId) -> Result<()>`

**Works:**

- `insert_work(&self, work: &Work) -> Result<()>`
- `get_work_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Work>>`
- `upsert_work(&self, work: &Work) -> Result<()>`

**Expressions:**

- `insert_expression(&self, expr: &Expression) -> Result<()>`
- `get_expression_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Expression>>`
- `upsert_expression(&self, expr: &Expression) -> Result<()>`

**Manifestations:**

- `insert_manifestation(&self, man: &Manifestation) -> Result<()>`
- `get_manifestation_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Manifestation>>`
- `upsert_manifestation(&self, man: &Manifestation) -> Result<()>`

**Artists:**

- `insert_artist(&self, artist: &Artist) -> Result<()>`
- `get_artist_by_musicbrainz_id(&self, mbid: &str) -> Result<Option<Artist>>`
- `upsert_artist(&self, artist: &Artist) -> Result<()>`

**Assertions:**

- `insert_assertion(&self, assertion: &Assertion) -> Result<()>`
- `get_assertions_for_entity(&self, entity_id: &str) -> Result<Vec<Assertion>>`
- `get_assertions_for_field(&self, entity_id: &str, field: &str) -> Result<Vec<Assertion>>`

#### 1.3.5: Thread safety consideration

`rusqlite::Connection` is `Send` but not `Sync`. For Phase 1 (single-threaded
CLI), this is fine — `Database` will be created in `main` and passed around
by `&mut` or owned reference. In later phases, if concurrent access is needed,
wrap it in a `Mutex` or use a connection pool.

### Acceptance Criteria

- [ ] `Database::open_in_memory()` succeeds and creates all tables
- [ ] Migrations are tracked in `schema_migrations` table
- [ ] Round-trip tests: insert an Item, read it back, verify all fields match
- [ ] Round-trip tests for Work, Expression, Manifestation, Artist
- [ ] `list_unidentified_items()` returns only items without an `expression_id`
- [ ] Foreign key constraints are enforced (inserting expression with bad work_id fails)
- [ ] Assertions can be stored and queried by entity/field
- [ ] `cargo test -p tessitura-core` passes with >= 90% coverage on schema module

---

## Milestone 1.4: Taxonomy Stubs

### Goal

Define type structures for genres, forms, periods, schools, and
instrumentation. These are stubs — the actual controlled vocabulary
loading comes in Phase 2.

### Where

`crates/tessitura-core/src/taxonomy/`

### Steps

#### 1.4.1: Create the taxonomy module

```
crates/tessitura-core/src/taxonomy/
├── mod.rs
├── genre.rs
├── form.rs
├── period.rs
└── instrumentation.rs
```

#### 1.4.2: Define types

**genre.rs:**

```rust
use serde::{Deserialize, Serialize};

/// A genre classification.
///
/// Genres can be hierarchical (e.g., "Classical > 20th Century").
/// In Phase 2, these will be mapped to LCGFT controlled vocabulary terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Genre {
    /// Display name (e.g., "20th Century Classical").
    pub name: String,

    /// Optional parent genre for hierarchical classification.
    pub parent: Option<String>,

    /// LCGFT URI, if mapped.
    pub lcgft_uri: Option<String>,
}

impl Genre {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: None,
            lcgft_uri: None,
        }
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}
```

**form.rs:**

```rust
use serde::{Deserialize, Serialize};

/// A musical form (sonata, fugue, rondo, string quartet, etc.).
///
/// Distinct from genre — form describes structure, genre describes style.
/// In Phase 2, these will be mapped to LCGFT form terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Form {
    pub name: String,
    pub lcgft_uri: Option<String>,
}

impl Form {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            lcgft_uri: None,
        }
    }
}
```

**period.rs:**

```rust
use serde::{Deserialize, Serialize};

/// A musical period or era.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Period {
    pub name: String,
    /// Approximate start year.
    pub start_year: Option<i32>,
    /// Approximate end year.
    pub end_year: Option<i32>,
}

impl Period {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start_year: None,
            end_year: None,
        }
    }

    pub fn with_range(mut self, start: i32, end: i32) -> Self {
        self.start_year = Some(start);
        self.end_year = Some(end);
        self
    }
}
```

**instrumentation.rs:**

```rust
use serde::{Deserialize, Serialize};

/// An instrument or medium of performance.
///
/// In Phase 2, these will be mapped to LCMPT controlled vocabulary terms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub abbreviation: Option<String>,
    pub lcmpt_uri: Option<String>,
}

impl Instrument {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            abbreviation: None,
            lcmpt_uri: None,
        }
    }
}
```

**mod.rs:**

```rust
pub mod form;
pub mod genre;
pub mod instrumentation;
pub mod period;

pub use form::Form;
pub use genre::Genre;
pub use instrumentation::Instrument;
pub use period::Period;
```

Update `crates/tessitura-core/src/lib.rs` to include:

```rust
pub mod taxonomy;
```

### Acceptance Criteria

- [ ] All taxonomy types compile and derive Debug, Clone, PartialEq, Serialize, Deserialize
- [ ] Builder patterns work for each type
- [ ] Types are re-exported from `tessitura_core::taxonomy`

---

## Milestone 1.5: Scan Stage

### Goal

Implement the Scan pipeline stage: walk a music directory tree, extract
embedded tags via lofty, compute fingerprints (stubbed initially), detect
new/changed/removed files, and create Item records in SQLite.

### Where

`crates/tessitura-etl/src/scan.rs`

### Dependencies

- Milestone 1.2 (model types)
- Milestone 1.3 (database)
- treadle crate

### Steps

#### 1.5.1: Define the treadle WorkItem for tessitura

Create `crates/tessitura-etl/src/work_item.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use treadle::WorkItem;

/// A music file being processed through the pipeline.
///
/// This is the treadle WorkItem that flows through the scan → identify
/// → enrich → harmonize → review → index → export stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicFile {
    /// Unique ID for this work item (matches the Item ID in the database).
    id: String,
    /// Path to the audio file.
    pub path: PathBuf,
}

impl MusicFile {
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
```

#### 1.5.2: Implement the ScanStage

The ScanStage implements `treadle::Stage`. It:

1. Walks the directory tree to find audio files (FLAC, MP3 at minimum)
2. For each file, extracts embedded tags using lofty
3. Records file metadata (size, mtime, format)
4. Creates or updates Item records in the database
5. Detects removed files (files in DB but not on disk)

```rust
use std::path::Path;
use treadle::{Stage, StageContext, StageOutcome};
use tessitura_core::model::{AudioFormat, Item};
use tessitura_core::schema::Database;

#[derive(Debug)]
pub struct ScanStage {
    music_dir: PathBuf,
    // Database reference will be passed via StageContext metadata
    // or held by the stage itself.
}
```

**Key implementation details:**

- Use `walkdir` or `std::fs::read_dir` recursively to find files.
  Add `walkdir = "2"` to workspace dependencies if used.
- Filter files by extension: `.flac`, `.mp3`, `.ogg`, `.wav`, `.m4a`
- Use `lofty::read_from_path()` to extract tags. Handle the `TaggedFile`
  and extract standard fields (title, artist, album, track number, etc.)
- For each file, compute: path, size, mtime, format, embedded tags
- Use `Database::get_item_by_path()` to check if the file is already known.
  If known and mtime hasn't changed, skip it. If mtime changed, re-extract
  and update. If new, insert.
- For fingerprinting: **stub it for now**. The `rusty-chromaprint` crate
  requires decoded PCM audio. In Phase 1, store `fingerprint: None` and
  add a TODO. Alternatively, if `rusty-chromaprint` is straightforward to
  integrate, include it — but it needs a decoder (like `symphonia`) to
  feed it PCM samples. Evaluate complexity and decide. If it's more than
  a day of work, stub it.

**Using lofty to extract tags:**

```rust
use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;

fn extract_tags(path: &Path) -> Result<TagData> {
    let tagged_file = lofty::read_from_path(path)?;

    // Get the primary tag (ID3v2 for MP3, VorbisComments for FLAC)
    let tag = tagged_file.primary_tag()
        .or_else(|| tagged_file.first_tag());

    let properties = tagged_file.properties();

    let tag_data = if let Some(tag) = tag {
        TagData {
            title: tag.title().map(|s| s.to_string()),
            artist: tag.artist().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            track_number: tag.track(),
            disc_number: tag.disk(), // note: lofty uses "disk"
            year: tag.year(),
            genre: tag.genre().map(|s| s.to_string()),
            duration_secs: properties.duration().as_secs_f64(),
            // album_artist requires reading a specific tag key
            album_artist: None, // see note below
        }
    } else {
        TagData::empty_with_duration(properties.duration().as_secs_f64())
    };

    Ok(tag_data)
}
```

**Note on album_artist:** lofty's `Accessor` trait doesn't have a direct
`album_artist()` method. You'll need to read it from the tag items directly:

```rust
// For ID3v2:
use lofty::id3::v2::FrameId;
// Read "TPE2" frame for album artist

// For Vorbis:
// Read "ALBUMARTIST" comment
```

Check the lofty docs for the exact approach — it varies by tag format.
A helper function that handles both ID3v2 and VorbisComments is appropriate.

#### 1.5.3: treadle Stage implementation

The `Stage::execute` method receives a `MusicFile` work item and a
`StageContext`. Since the Scan stage processes an entire directory
(not a single file), the work item for the scan stage is somewhat
special — it represents "the scan job" rather than an individual file.

**Design choice:** There are two reasonable approaches:

1. **One WorkItem per directory** — the ScanStage processes the whole
   directory and creates Item records. Downstream stages (Identify,
   Enrich) receive individual file WorkItems.

2. **One WorkItem per file** — the scan stage itself is responsible for
   discovering files and creating WorkItems for each one.

**Recommended: Approach 1.** Use a single `ScanJob` WorkItem for the
directory scan, then create individual `MusicFile` WorkItems for each
discovered file to flow through the rest of the pipeline. The scan stage
returns `StageOutcome::Complete` after processing the directory.

The individual `MusicFile` items are then advanced through subsequent
stages (identify, enrich, etc.) by the executor.

#### 1.5.4: Change detection

For detecting changed files, compare the stored `file_mtime` against the
current file's modification time. If they differ, re-extract tags and
update the record. For detecting removed files, query all items in the
database and check if their paths still exist on disk. Mark removed files
(either delete them or add a `removed` status field — prefer a status
field so data isn't lost).

### Acceptance Criteria

- [ ] `ScanStage` implements `treadle::Stage`
- [ ] Scanning a test directory discovers FLAC and MP3 files
- [ ] Embedded tags are correctly extracted via lofty
- [ ] Item records are created in SQLite with all tag fields populated
- [ ] Re-scanning the same directory is idempotent (no duplicates)
- [ ] Changed files (different mtime) are re-extracted and updated
- [ ] Files removed from disk are detected
- [ ] Unit tests with real test audio files (short clips) in `tests/fixtures/`
- [ ] Integration test: scan a directory, verify DB contents

---

## Milestone 1.6: Identify Stage

### Goal

Implement the Identify pipeline stage: submit fingerprints to AcoustID to
get MusicBrainz recording IDs, fall back to metadata-based matching, and
link Items to MB entities in the schema.

### Where

`crates/tessitura-etl/src/identify.rs`

### Dependencies

- Milestone 1.5 (scan stage populates Items)
- Milestone 1.3 (database for reading/writing Items, creating Works/Expressions/Manifestations)

### Steps

#### 1.6.1: AcoustID client

Create `crates/tessitura-etl/src/acoustid.rs` for the AcoustID API client.

**AcoustID API:**

- Endpoint: `https://api.acoustid.org/v2/lookup`
- Method: POST
- Parameters:
  - `client` — API key (required, free registration)
  - `fingerprint` — chromaprint fingerprint
  - `duration` — track duration in seconds
  - `meta` — what metadata to return: `recordings releases releasegroups`
- Rate limit: 3 requests/second

```rust
use reqwest::Client;
use serde::Deserialize;

pub struct AcoustIdClient {
    http: Client,
    api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdResponse {
    pub status: String,
    pub results: Vec<AcoustIdResult>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdResult {
    pub id: String,
    pub score: f64,
    pub recordings: Option<Vec<AcoustIdRecording>>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdRecording {
    pub id: String,                    // MusicBrainz recording ID
    pub title: Option<String>,
    pub artists: Option<Vec<AcoustIdArtist>>,
    pub releases: Option<Vec<AcoustIdRelease>>,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdArtist {
    pub id: String,                    // MusicBrainz artist ID
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AcoustIdRelease {
    pub id: String,                    // MusicBrainz release ID
    pub title: Option<String>,
}
```

Implement `lookup` with retry logic using `backon`:

```rust
impl AcoustIdClient {
    pub async fn lookup(
        &self,
        fingerprint: &str,
        duration: f64,
    ) -> Result<AcoustIdResponse> {
        // Use backon for exponential backoff retry
        // Respect rate limit (3 req/s) with a simple delay
    }
}
```

#### 1.6.2: MusicBrainz client (basic)

Create `crates/tessitura-etl/src/musicbrainz.rs` for basic MB API calls.
In Phase 1, we only need recording lookup to get the work and release
linkage. Full enrichment comes in Phase 2.

**MusicBrainz API:**

- Base URL: `https://musicbrainz.org/ws/2/`
- Format: JSON (`?fmt=json`)
- Rate limit: **1 request/second** (strict — they're a nonprofit)
- **Required:** User-Agent header: `tessitura/0.1.0 (https://github.com/oxur/tessitura)`

Key endpoints for Phase 1:

- `GET /recording/{mbid}?inc=releases+artists+work-rels` — get recording
  details with linked releases, artists, and work relationships

```rust
pub struct MusicBrainzClient {
    http: Client,
}

impl MusicBrainzClient {
    pub async fn get_recording(&self, mbid: &str) -> Result<MbRecording> {
        // GET /ws/2/recording/{mbid}?inc=releases+artists+work-rels&fmt=json
        // Rate limited to 1 req/sec
    }
}
```

Define response types for the subset of MB data we need in Phase 1.
The MB API returns deeply nested JSON — only deserialize the fields
we actually use. Use `#[serde(default)]` liberally.

#### 1.6.3: Implement the IdentifyStage

The IdentifyStage implements `treadle::Stage`. For each unidentified Item:

1. If the item has a fingerprint, submit to AcoustID → get MB recording ID
2. If AcoustID fails or no fingerprint, fall back to metadata matching:
   search MusicBrainz by artist + album + track title
3. With the MB recording ID, fetch recording details from MusicBrainz
4. Create or update Work, Expression, Manifestation records in the database
5. Link the Item to its Expression and Manifestation

```rust
#[derive(Debug)]
pub struct IdentifyStage {
    acoustid: AcoustIdClient,
    musicbrainz: MusicBrainzClient,
}

impl Stage for IdentifyStage {
    fn name(&self) -> &str {
        "identify"
    }

    async fn execute(
        &self,
        item: &dyn WorkItem,
        context: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        // 1. Load the Item from the database
        // 2. Try AcoustID lookup (if fingerprint present)
        // 3. Fall back to metadata search
        // 4. Create/link FRBR entities
        // 5. Update Item with expression_id and manifestation_id
        Ok(StageOutcome::Complete)
    }
}
```

#### 1.6.4: Metadata-based fallback matching

When AcoustID fingerprint is unavailable or returns no results, search
MusicBrainz using embedded tag metadata:

```
GET /ws/2/recording/?query=recording:{title} AND artist:{artist} AND release:{album}&fmt=json
```

This is a Lucene query search. Parse the results and take the best match
above a confidence threshold.

#### 1.6.5: Entity creation logic

When we identify a recording via MusicBrainz, we need to create (or find
existing) entities in our database:

1. **Artist**: Check if artist exists by MB ID, create if not
2. **Work**: If the recording has work relationships (common for classical),
   check if work exists by MB ID, create if not
3. **Expression**: Create an Expression linked to the Work, with the MB
   recording ID
4. **Manifestation**: Check if release exists by MB ID, create if not
5. **Item**: Update the existing Item with expression_id and manifestation_id

Use `upsert` semantics — if an entity already exists by MB ID, update it
rather than creating a duplicate.

#### 1.6.6: Configuration

The AcoustID API key must be configurable. Options:

1. Environment variable: `ACOUSTID_API_KEY`
2. Config file: `~/.config/tessitura/config.toml`

For Phase 1, environment variable is sufficient. Create a simple config
struct:

```rust
pub struct Config {
    pub acoustid_api_key: Option<String>,
    pub database_path: PathBuf,
    pub music_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            acoustid_api_key: std::env::var("ACOUSTID_API_KEY").ok(),
            database_path: default_db_path(),
            music_dir: PathBuf::new(), // set from CLI args
        }
    }
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tessitura")
        .join("tessitura.db")
}
```

Add `dirs = "6"` to workspace dependencies for platform-appropriate
data directory resolution.

### Acceptance Criteria

- [ ] `IdentifyStage` implements `treadle::Stage`
- [ ] AcoustID lookup works with a valid API key and fingerprint
- [ ] Metadata-based fallback search works for files without fingerprints
- [ ] Work, Expression, Manifestation, and Artist records are created in DB
- [ ] Items are linked to their Expression and Manifestation after identification
- [ ] Rate limiting is respected (1 req/s for MusicBrainz, 3 req/s for AcoustID)
- [ ] Retry logic handles transient HTTP errors
- [ ] Integration tests with mocked HTTP responses (use `mockito` or similar)
- [ ] Items that cannot be identified are left with `expression_id: None`

---

## Milestone 1.7: Pipeline Wiring

### Goal

Wire the Scan and Identify stages into a treadle Workflow. Implement the
CLI commands `tessitura scan`, `tessitura identify`, and `tessitura status`.

### Where

- `crates/tessitura-etl/src/pipeline.rs` — workflow construction
- `crates/tessitura-cli/src/main.rs` — CLI commands

### Dependencies

- Milestone 1.5 (ScanStage)
- Milestone 1.6 (IdentifyStage)
- treadle crate

### Steps

#### 1.7.1: Build the treadle Workflow

```rust
use treadle::Workflow;
use crate::scan::ScanStage;
use crate::identify::IdentifyStage;

pub fn build_pipeline(
    scan_stage: ScanStage,
    identify_stage: IdentifyStage,
) -> treadle::Result<Workflow> {
    Workflow::builder()
        .stage("scan", scan_stage)
        .stage("identify", identify_stage)
        .dependency("identify", "scan")
        .build()
}
```

#### 1.7.2: Implement the CLI commands

Update `crates/tessitura-cli/src/main.rs` to wire up real implementations:

**`tessitura scan /path/to/music`:**

1. Open (or create) the SQLite database
2. Create a `ScanStage` for the given path
3. Create a treadle `SqliteStateStore` (or `MemoryStateStore` for now)
4. Create a `ScanJob` work item
5. Build the workflow with just the scan stage
6. Call `workflow.advance(&scan_job, &mut store)`
7. Print summary: N files found, M new, K updated, J removed

**`tessitura identify`:**

1. Open the database
2. List all unidentified items
3. For each unidentified item, create a `MusicFile` work item
4. Build the workflow with scan + identify stages
5. Advance each work item (scan will be skipped since items exist)
6. Print summary: N identified, M unidentified

**`tessitura status`:**

1. Open the database
2. For each work item, call `workflow.status(item_id, &store)`
3. Print a summary table showing pipeline progress per item/album

#### 1.7.3: Event stream for progress display

Subscribe to the workflow's event stream to show real-time progress:

```rust
let mut events = workflow.subscribe();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match event {
            WorkflowEvent::StageStarted { item_id, stage } => {
                println!("  [{stage}] Processing: {item_id}");
            }
            WorkflowEvent::StageCompleted { item_id, stage } => {
                println!("  [{stage}] Complete: {item_id}");
            }
            WorkflowEvent::StageFailed { item_id, stage, error } => {
                eprintln!("  [{stage}] FAILED: {item_id}: {error}");
            }
            _ => {}
        }
    }
});
```

#### 1.7.4: Database and state store paths

- Tessitura's SQLite database (FRBR model): `~/.local/share/tessitura/tessitura.db`
- Treadle's SQLite state store (pipeline state): `~/.local/share/tessitura/pipeline.db`

These are separate databases — tessitura's data model and treadle's
workflow state are independent concerns.

#### 1.7.5: Organize CLI modules

As the CLI grows, split into modules:

```
crates/tessitura-cli/src/
├── main.rs
├── commands/
│   ├── mod.rs
│   ├── scan.rs
│   ├── identify.rs
│   └── status.rs
└── display.rs        # Table formatting helpers
```

### Acceptance Criteria

- [ ] `tessitura scan /path/to/music` scans the directory and creates Item records
- [ ] `tessitura identify` identifies unidentified items via AcoustID/MusicBrainz
- [ ] `tessitura status` shows pipeline progress per item
- [ ] The two stages are wired as a treadle Workflow with scan → identify dependency
- [ ] Progress events are displayed during processing
- [ ] Database is created automatically if it doesn't exist
- [ ] `--help` works for all subcommands

---

## Milestone 1.8: Test Album Validation

### Goal

Process 3-5 real albums end-to-end through scan → identify. Validate that
MusicBrainz identification succeeds, the schema is populated correctly,
and state tracking works.

### Test Albums

These are suggested, covering genre diversity. Use whichever albums the
user has available:

1. **Classical:** Bartók String Quartets (Takács Quartet, Decca 455 297-2)
2. **Jazz:** Miles Davis — Kind of Blue
3. **Electronic:** Boards of Canada — Music Has the Right to Children
4. **Prog:** King Crimson — In the Court of the Crimson King
5. **Contemporary:** user's choice from Berklee studies

### Steps

#### 1.8.1: Run the full pipeline on test albums

```sh
export ACOUSTID_API_KEY="your-key-here"

# Scan
tessitura scan /path/to/test-music

# Identify
tessitura identify

# Check status
tessitura status
```

#### 1.8.2: Validate database contents

After running, verify:

- All audio files are in the `items` table
- Tags were correctly extracted (compare with what a tool like `ffprobe` reports)
- MusicBrainz identification succeeded for well-known albums
- Work, Expression, Manifestation, Artist records were created
- Items are linked to their Expressions and Manifestations
- For the Bartók album specifically: verify that Work records distinguish
  between the six quartets (not merged into one)

#### 1.8.3: Document issues

Record any issues encountered:

- Fingerprinting accuracy (what percentage of tracks were identified?)
- MusicBrainz coverage gaps (any albums not in MB?)
- Classical music challenges (are Works correctly distinguished from
  Expressions? Is the conductor properly captured?)
- Tag extraction issues (any fields missing or malformed?)
- Performance (how long does scan + identify take for N tracks?)

#### 1.8.4: Create integration tests

Create automated integration tests using test fixtures (short audio clips
or mocked responses) that verify the full pipeline works:

```rust
#[tokio::test]
async fn test_scan_and_identify_pipeline() {
    // Use test fixtures with known content
    // Mock AcoustID and MusicBrainz responses
    // Verify database state after pipeline completes
}
```

### Acceptance Criteria

- [ ] At least 3 real albums processed end-to-end
- [ ] MusicBrainz identification succeeds for well-known albums
- [ ] Database contains correct Work/Expression/Manifestation/Item hierarchy
- [ ] Classical album has distinct Work records per composition
- [ ] Issues are documented for future phases
- [ ] Automated integration tests exist using mocked APIs

---

## Appendix A: Dependency Checklist

Final workspace dependency list for Phase 1 (verify versions before implementing):

| Crate | Purpose | Used By |
|---|---|---|
| `tokio` (full) | Async runtime | etl, cli |
| `serde` + `serde_json` | Serialization | core, etl |
| `thiserror` | Error types | core, etl, graph, search |
| `anyhow` | CLI error handling | cli |
| `tracing` + `tracing-subscriber` | Logging | etl, cli |
| `chrono` | Timestamps | core, etl |
| `clap` (derive) | CLI parsing | cli |
| `uuid` (v4) | ID generation | core |
| `rusqlite` (bundled) | SQLite | core |
| `treadle` | Workflow engine | etl |
| `lofty` | Audio tag read | etl |
| `reqwest` (json) | HTTP client | etl |
| `backon` | Retry with backoff | etl |
| `walkdir` | Directory walking | etl |
| `dirs` | Platform dirs | cli |
| `tempfile` | Test helpers | dev |

## Appendix B: Treadle API Quick Reference

### Core traits

- **`WorkItem`** — `fn id(&self) -> &str`
- **`Stage`** — `fn name(&self) -> &str`, `async fn execute(&self, item: &dyn WorkItem, ctx: &mut StageContext) -> Result<StageOutcome>`
- **`StateStore`** — async trait for persistence (7 methods)

### Building a workflow

```rust
let workflow = Workflow::builder()
    .stage("scan", scan_stage)
    .stage("identify", identify_stage)
    .dependency("identify", "scan")
    .build()?;
```

### Executing

```rust
workflow.advance(&work_item, &mut store).await?;
```

### Checking status

```rust
let status = workflow.status(item_id, &store).await?;
println!("{status}");  // Has Display impl with progress bar
```

### StageOutcome variants

- `Complete` — proceed to next stage
- `NeedsReview` — pause for human review (used in Phase 2)
- `Retry` — failed, should be retried
- `Failed` — failed permanently
- `FanOut(Vec<SubTask>)` — fan out into subtasks (used in Phase 2)

### State stores

- `SqliteStateStore::open(path).await?` — persistent
- `SqliteStateStore::open_in_memory().await?` — for tests
- `MemoryStateStore::new()` — in-memory, for unit tests
