---
number: 1
title: "Tessitura — Project Plan"
author: "other tools"
component: All
tags: [change-me]
created: 2026-02-07
updated: 2026-02-07
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# Tessitura — Project Plan

**A musicological library cataloging tool for serious musicians, built in Rust.**

> Version: 0.1-draft
> Last Updated: 2026-02-07
> Status: Pre-development — architecture and design phase complete

---

## Table of Contents

1. [Vision and Motivation](#1-vision-and-motivation)
2. [Background: The Problem Space](#2-background-the-problem-space)
3. [Key Design Decisions](#3-key-design-decisions)
4. [Architecture Overview](#4-architecture-overview)
5. [The Superset Schema: Library Science Meets Audio Tags](#5-the-superset-schema-library-science-meets-audio-tags)
6. [The ETL Pipeline](#6-the-etl-pipeline)
7. [Treadle: The Workflow Engine We Had to Build First](#7-treadle-the-workflow-engine-we-had-to-build-first)
8. [Crate Dependencies and Ecosystem Choices](#8-crate-dependencies-and-ecosystem-choices)
9. [Project Structure](#9-project-structure)
10. [Development Phases](#10-development-phases)
11. [Open Questions and Future Directions](#11-open-questions-and-future-directions)

---

## 1. Vision and Motivation

Tessitura is a CLI-first tool for cataloging, enriching, and searching a personal digital music library with musicological rigor. It is designed for **serious musicians, composition students, musicologists, and archivists** — people for whom "Genre: Classical" is not remotely sufficient.

The name comes from *tessitura* — the vocal or instrumental range where a performer is most comfortable and expressive. The metaphor: finding the range where your music collection is most naturally discoverable and useful.

### The User

The motivating user is a CTO by day, Berklee Online composition student by night, with a moderate digital music collection spanning classical, jazz, electronic, and prog rock. They need to find specific performances (not just compositions) for academic study. Consumer tools like Plex, MusicBrainz Picard, and streaming services cannot provide the granularity required.

### What Tessitura Is Not

Tessitura is not a music player, not a streaming service, not a replacement for Plex or any media server. It is a **cataloging, enrichment, and search tool** that can optionally write enriched metadata back into audio files (FLAC, MP3) for consumption by other tools, and that can optionally deep-link into Plex for playback.

---

## 2. Background: The Problem Space

### Why Existing Tools Fall Short

**MusicBrainz Picard** excels at identifying releases and fixing basic metadata (artist, album, year) via acoustic fingerprinting and a large community database. However, its genre tagging relies on community folksonomy tags that are inconsistent, especially for classical and contemporary music. Picard's tag-writing is also lossy relative to what the MusicBrainz database actually knows — the data model is richer than what ends up in your files.

**Beets** is a command-line music library manager with a plugin architecture. It can pull from MusicBrainz and other sources. The `lastgenre` plugin fetches Last.fm tags. However, beets is Python-based, its classical music handling is a known weak point, and it doesn't provide the musicological depth we need (form, instrumentation, period, school, catalog numbers as first-class fields).

**Plex** reads embedded tags from audio files and provides a decent playback experience, but its genre classification is flat and unsophisticated. It has limited support for classical-specific fields even when they're present in tags. Plex will remain the user's playback tool, but tessitura will be the cataloging and search layer.

### The Fundamental Problem

The core issue is that **no consumer audio tagging standard can express the full relational model of music metadata**. ID3v2 and Vorbis comments give you flat key-value pairs. There's no native way to express:

> "This is a performance (expression) of Bartók's String Quartet No. 4 (work), III. Non troppo lento (part), performed by the Takács Quartet (ensemble), in a 1998 Decca recording (manifestation), which I own as a FLAC rip (item)."

You end up flattening a rich relational model into flat tags and hoping your player software can reconstruct something useful. Tessitura solves this by maintaining the full relational model internally and projecting a flattened version into audio tags when needed.

### The Library Science Foundation

The FRBR (Functional Requirements for Bibliographic Records) hierarchy, used in modern library cataloging (RDA), maps beautifully to music:

| FRBR Level | Music Example |
|---|---|
| **Work** | Beethoven's Symphony No. 5 |
| **Expression** | Karajan/BPO 1962 interpretation |
| **Manifestation** | The specific DG LP release (catalog #) |
| **Item** | Your FLAC rip of that release |

This hierarchy is the conceptual backbone of tessitura's data model.

---

## 3. Key Design Decisions

These decisions were made during the design phase and should be treated as settled unless compelling new information emerges.

### Language: Rust

Non-negotiable. The user is an experienced Rust developer. The tool will be open-sourced. Rust provides the performance, safety, and ecosystem needed for a tool that processes audio files, manages a SQLite database, runs an in-memory graph, and potentially serves an HTTP API.

### Interface: CLI-first with TUI for review workflows

The primary interface is a CLI with structured table output (`tessitura scan`, `tessitura enrich`, `tessitura review`, `tessitura search`). The review workflow — where the user approves/edits/rejects enrichment proposals — will use a ratatui-based TUI. A future phase adds an HTTP server with a web UI.

### Data Model: FRBR-rooted relational model in SQLite

SQLite is the durable store and source of truth. The schema follows the FRBR Work/Expression/Manifestation/Item hierarchy, extended with a superset of metadata fields drawn from multiple library science standards (see Section 5).

### Search: petgraph (structured) + LanceDB (vector/fuzzy)

An in-memory petgraph graph is hydrated from SQLite on startup. Nodes are works, artists, recordings, genres, forms, instruments, periods. Edges are typed relationships. This enables structured queries like "find all 20th-century chamber works for strings in a minor key." LanceDB provides vector similarity search for fuzzy/associative queries like "find things that sound like this Feldman piece." The two can cross-reference each other.

### Pipeline: treadle (custom workflow engine)

The ETL pipeline is powered by `treadle`, a standalone workflow engine crate we built specifically because nothing in the Rust ecosystem provided persistent state + resumability + human-in-the-loop review gates + fan-out with per-subtask tracking. See Section 7.

### Audio Fingerprinting: rusty-chromaprint (pure Rust)

`rusty-chromaprint` is a pure Rust port of chromaprint for the AcoustID project. No C FFI, no system library dependency. This is used in the Identify stage to match audio files against the AcoustID/MusicBrainz database.

### Tag Reading/Writing: lofty-rs

`lofty` is the current best Rust crate for reading and writing audio metadata across formats (ID3v2 for MP3, Vorbis comments for FLAC/OGG, etc.).

### Metadata Granularity: Maximum, with layered/compound search

The user wants the most granular classification possible: not just "Classical" but "Chamber Music > String Quartet > 20th Century > Hungarian Modernism." Multiple genre/form/period tags per track, composable via AND/OR in search queries. Classical-specific fields (composer, conductor, orchestra, opus/catalog numbers, key, instrumentation, form) are first-class.

---

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  CLI / TUI Interface                │
│          (structured queries + fuzzy search         │
│           + review workflow + status display)       │
└──────────┬──────────────────────┬───────────────────┘
           │                      │
     ┌─────v───────┐        ┌─────v──────┐
     │  petgraph   │◄──────►│  LanceDB   │
     │ (structured │        │ (vector    │
     │  queries)   │        │  search)   │
     └─────^───────┘        └─────^──────┘
           │                      │
     ┌─────v──────────────────────v───────────────────┐
     │          Canonical Schema (SQLite)             │
     │   FRBR hierarchy + superset metadata           │
     │   + provenance tracking per assertion          │
     └────────────────────^───────────────────────────┘
                          │
     ┌────────────────────v───────────────────────────┐
     │       ETL Pipeline (powered by treadle)        │
     │                                                │
     │  scan ──► identify ──► enrich ──► harmonize    │
     │                          │          │          │
     │                   ┌──────v──────┐   v          │
     │                   │  fan-out    │ review       │
     │                   │             │   │          │
     │                   │ MB  Wiki    │   v          │
     │                   │ Last Disc   │ index        │
     │                   │ LCGFT       │   │          │
     │                   └─────────────┘   v          │
     │                                   export       │
     └────────────────────────────────────────────────┘
```

### Data Flow

1. **Scan** — walk the filesystem, fingerprint audio files, extract existing embedded tags, detect new/changed/removed files.
2. **Identify** — match against AcoustID/MusicBrainz via fingerprint and/or metadata. This yields MB work ID, recording ID, release ID — anchors into the knowledge graph.
3. **Enrich** — fan out to multiple sources concurrently: MusicBrainz (detail), Wikidata, Last.fm (folksonomy), LCGFT/LCMPT (controlled vocabularies), Discogs (release details). Each source writes assertions with provenance.
4. **Harmonize** — apply curated mapping rules to normalize genres, forms, and periods into the user's controlled vocabulary. Resolve conflicts. Flag ambiguities.
5. **Review** — human-in-the-loop approval. The user sees proposed tags, conflict resolutions, and source provenance. Can accept all, review track-by-track, or edit mapping rules.
6. **Index** — update the petgraph and LanceDB from the canonical store.
7. **Export** — write tags back into audio files (lossy projection of the rich model into flat tags, optimized for Plex).

---

## 5. The Superset Schema: Library Science Meets Audio Tags

Tessitura pulls metadata from multiple sources, each with different strengths:

| Source | Strongest At | Access Method |
|---|---|---|
| MusicBrainz | Work/Recording/Release relationships, artist credits, catalog numbers | REST API (1 req/sec rate limit) |
| Wikidata | Structured properties — key, form, opus, instrumentation, period, school | SPARQL / REST API |
| LCGFT | Controlled genre/form vocabulary with hierarchical broader/narrower terms | Published as SKOS/RDF linked data |
| LCMPT | Controlled instrumentation vocabulary | Published as SKOS/RDF linked data |
| Last.fm | Folksonomy tags — messy but captures "vibe" and cross-genre associations | REST API |
| Discogs | Release-level detail, label info, pressings, personnel | REST API |

### Provenance Tracking

Every metadata assertion in the canonical store includes provenance: which source provided it, when it was fetched, and what confidence level. This allows the harmonization layer to arbitrate conflicts: MusicBrainz says "Electronic," Last.fm says "ambient, drone, dark ambient," LCGFT says "Electroacoustic music" — the mapping rules decide what the canonical representation is, but the raw assertions are preserved.

### LC Controlled Vocabularies

LCGFT (Library of Congress Genre/Form Terms) and LCMPT (Library of Congress Medium of Performance Thesaurus) are published as linked data and updated periodically. Tessitura ships a snapshot of these vocabularies as reference data and supports refreshing them. LCGFT critically distinguishes between *genre* (jazz, electronic) and *form* (sonata, fugue, rondo) — exactly the layering needed for musicological search.

### Classical-Specific Fields

The schema includes first-class support for: composer (distinct from performer/artist), conductor, orchestra/ensemble, opus number, catalog number (BWV, K., Sz., BB., etc.), musical key, form (sonata, string quartet, symphony, etc.), movement structure, instrumentation (using LCMPT vocabulary), period (Baroque, Classical, Romantic, 20th Century, Contemporary), and school/movement (Second Viennese School, Hungarian Modernism, Minimalism, etc.).

---

## 6. The ETL Pipeline

### Pipeline Stages in Detail

**Stage 1: Scan**
Walk the user's music directory tree. For each audio file: extract existing embedded tags (using lofty-rs), compute acoustic fingerprint (using rusty-chromaprint), record file path, format, size, modification time. Detect new files, changed files (by mtime/hash), and removed files. Output: a set of FileRecord entries in the canonical store.

**Stage 2: Identify**
For each unidentified file: submit fingerprint to AcoustID to get a MusicBrainz recording ID. Fall back to metadata-based matching (artist + album + track name) if fingerprinting fails. Link the file to MB recording, release, and work entities. Output: MB entity IDs associated with each file.

**Stage 3: Enrich (fan-out)**
Given MB IDs, fan out to multiple sources concurrently. Each source is a subtask tracked independently by treadle:

- **MusicBrainz detail**: full recording, release, and work metadata including artist relationships, track listings, labels.
- **Wikidata**: structured properties linked via MB IDs — key, form, opus, instrumentation, period, school.
- **Last.fm**: folksonomy tags for the artist and track.
- **LCGFT/LCMPT**: map from MB/Wikidata genre and instrumentation data to controlled vocabulary terms.
- **Discogs**: release-level detail, personnel, label information.

Each source writes its assertions with provenance. If a source fails, only that subtask is retried.

**Stage 4: Harmonize**
Apply the user's curated mapping rules to produce canonical metadata:

- Normalize genre tags to a controlled vocabulary (e.g., Last.fm "modern classical, 20th century" → "Classical > 20th Century")
- Map instrumentation strings to LCMPT terms
- Resolve conflicts between sources (with configurable priority ordering)
- Flag remaining ambiguities for human review

The mapping rules are stored as configuration (likely TOML or YAML) and are the primary mechanism for the system to "learn" the user's preferences over time.

**Stage 5: Review (human-in-the-loop)**
The pipeline pauses here. The user is presented with proposed metadata for each album/track, including conflict resolutions and source provenance. Two-level view:

- **Album list**: all albums awaiting review, with conflict counts for triage
- **Track detail**: per-track proposed tags, conflicts shown with resolution rationale, source provenance visible

The user can: accept all (for albums with zero conflicts), review track-by-track, edit individual values, or edit the mapping rules so the same conflict resolves correctly in the future.

**Stage 6: Index**
After review approval, update the in-memory petgraph and LanceDB vector index from the canonical store.

**Stage 7: Export**
Optionally write enriched metadata back into audio files. This is a lossy projection — the rich relational model is flattened into the best possible representation in ID3v2/Vorbis comments, optimized for what Plex reads and displays.

### Review Workflow UX

```
$ tessitura review

┌───────────────────────────────────────────────────────────┐
│  Albums Awaiting Review                           12 new  │
├────┬────────────────────────┬───────────────┬─────────────┤
│  # │ Album                  │ Artist        │ Status      │
├────┼────────────────────────┼───────────────┼─────────────┤
│  1 │ String Quartets 1-6    │ Bartók/Takács │ 6/6 pending │
│  2 │ Kind of Blue           │ Miles Davis   │ 5/5 pending │
│  3 │ Music Has the Right…   │ BoC           │ 8/8 pending │
│  4 │ Déserts                │ Varèse/Boulez │ 3/3 pending │
│ .. │ ...                    │               │             │
└────┴────────────────────────┴───────────────┴─────────────┘
Select album [1-12]: 1
```

Drilling into an album shows per-track proposed tags with conflicts surfaced explicitly:

```
┌─────────────────────────────────────────────────────────────────────┐
│  String Quartets 1-6 — Bartók — Takács Quartet                      │
│  Release: Decca 455 297-2 (1998)                                    │
│  Sources: MusicBrainz ✓  Wikidata ✓  Last.fm ✓  LCGFT ✓             │
├────┬────────────────────────┬───────────────────────────────────────┤
│  # │ Track                  │ Proposed Tags                         │
├────┼────────────────────────┼───────────────────────────────────────┤
│  1 │ SQ No.1, I. Lento      │ form: String Quartet                  │
│    │                        │ genre: Classical > 20th Century       │
│    │                        │ key: A minor                          │
│    │                        │ instrumentation: 2vn, va, vc          │
│    │                        │ catalog: Sz.40, BB.52                 │
│    │                        │ ── conflicts ──                       │
│    │                        │ genre: MB="Classical"                 │
│    │                        │   Last.fm="modern classical, 20th c." │
│    │                        │   LCGFT="String quartets"             │
│    │                        │ → resolved: Classical > 20th Century  │
├────┼────────────────────────┼───────────────────────────────────────┤
│  2 │ SQ No.1, II. Allegro   │ (similar, key differs)                │
└────┴────────────────────────┴───────────────────────────────────────┘
[a]ccept all  [t]rack-by-track  [e]dit rules  [s]kip  [q]uit
```

### Pipeline Status Display

```
$ tessitura status "String Quartets 1-6"

scan ──✓──► identify ──✓──► enrich ──✓──► harmonize ──⏳──► index ──○──► export
                              │
                    MB ✓  Wiki ✓  Last.fm ✓  LCGFT ✓  Discogs ✗(retry 2)
```

---

## 7. Treadle: The Workflow Engine We Had to Build First

### Why We Needed a New Crate

During design, we surveyed the Rust ecosystem for workflow/DAG execution libraries and found a clear gap. The existing options fall into two categories:

**Single-shot DAG executors** (dagrs, dagx, async_dag): Define tasks, run them in parallel, get results. No persistent state, no pause/resume, no human-in-the-loop, no concept of work items progressing over time.

**Heavyweight distributed engines** (Restate, Temporal, Flawless): Durable execution with replay journals and distributed state. Require external runtime servers. Designed for microservices at scale. Massive overkill for a personal CLI tool.

Nothing provided: persistent state per work-item-per-stage (SQLite), pause/resume with human review gates, fan-out with per-subtask visibility and independent retry, single-process embeddable library (no external runtime), and an event stream for real-time TUI/CLI observation.

### What Treadle Provides

Treadle is a standalone crate (published to crates.io) that tessitura depends on. It provides:

1. **DAG definition** — petgraph-backed, with cycle detection at build time
2. **Work item state machine** — Pending → Running → Completed/Failed/AwaitingReview, per stage, per item
3. **Persistent state store trait** — with SQLite implementation (and in-memory for tests)
4. **Event stream** — tokio broadcast channel for real-time observation
5. **Fan-out** — per-subtask state tracking with independent retry
6. **Executor** — walks items through stages in topological order, respects dependencies, pauses at review gates

Treadle knows nothing about music. It is a generic workflow engine. Tessitura implements the music-specific stages using tessitura-core types.

### Treadle Repository

Published as `treadle` on crates.io. Repository: (see treadle repo). MIT OR Apache-2.0 licensed. The full design rationale and comparison with related projects is in treadle's README.

---

## 8. Crate Dependencies and Ecosystem Choices

### Core Stack (settled)

| Crate | Purpose | Notes |
|---|---|---|
| `tokio` | Async runtime | Full features |
| `serde` + `serde_json` | Serialization | Used throughout |
| `thiserror` | Error types | For library crates |
| `anyhow` | Error handling | For binary/CLI crate |
| `tracing` + `tracing-subscriber` | Structured logging | |
| `chrono` | Date/time | With serde feature |
| `clap` | CLI argument parsing | With derive feature |

### Domain-Specific (settled)

| Crate | Purpose | Notes |
|---|---|---|
| `rusty-chromaprint` | Audio fingerprinting | Pure Rust, no FFI. ~3K SLoC, MIT. |
| `lofty` | Audio tag read/write | Multi-format: ID3v2, Vorbis, etc. |
| `rusqlite` | SQLite | With bundled feature |
| `petgraph` | Graph data structure | For both treadle DAG and music knowledge graph |
| `lancedb` | Vector database | For similarity/fuzzy search |
| `ratatui` + `crossterm` | TUI | For review workflow |

### Resilience (settled)

| Crate | Purpose | Notes |
|---|---|---|
| `backon` | Retry with backoff | For API calls |
| `failsafe` | Circuit breaker | Per enrichment source |
| `reqwest` | HTTP client | For MusicBrainz, Wikidata, Last.fm, Discogs APIs |

### Future Phase

| Crate | Purpose | Notes |
|---|---|---|
| `axum` | HTTP server | For daemon mode / web UI |
| `notify` | File watcher | For daemon mode auto-scan |

---

## 9. Project Structure

```
tessitura/
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── tessitura-core/           # Domain model, FRBR types, schema
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model/            # Work, Expression, Manifestation, Item
│   │       ├── schema/           # SQLite schema, migrations
│   │       ├── taxonomy/         # Controlled vocabularies, mapping rules
│   │       └── provenance.rs     # Source tracking for assertions
│   │
│   ├── tessitura-etl/            # Pipeline stage implementations
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scan.rs           # File walking + tag extraction + fingerprinting
│   │       ├── identify.rs       # AcoustID / MusicBrainz matching
│   │       ├── enrich/           # Fan-out enrichment sources
│   │       │   ├── mod.rs
│   │       │   ├── musicbrainz.rs
│   │       │   ├── wikidata.rs
│   │       │   ├── lastfm.rs
│   │       │   ├── lcgft.rs
│   │       │   └── discogs.rs
│   │       ├── harmonize.rs      # Mapping rules engine
│   │       ├── index.rs          # Graph + vector index update
│   │       └── export.rs         # Tag writing back to files
│   │
│   ├── tessitura-graph/          # petgraph-based music knowledge graph
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── build.rs          # Hydrate graph from SQLite
│   │       ├── query.rs          # Structured graph queries
│   │       └── display.rs        # Graph visualization helpers
│   │
│   ├── tessitura-search/         # LanceDB vector search
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── embed.rs          # Generate embeddings from metadata
│   │       ├── index.rs          # LanceDB index management
│   │       └── query.rs          # Similarity search with structured filters
│   │
│   └── tessitura-cli/            # CLI + TUI entry point
│       └── src/
│           ├── main.rs           # clap CLI
│           ├── commands/         # Subcommands: scan, identify, enrich, review, search, status
│           ├── tui/              # ratatui review workflow
│           └── display/          # Table formatting, status rendering
│
├── config/                       # Default mapping rules, vocabulary snapshots
│   ├── taxonomy.toml             # Genre/form mapping rules
│   ├── lcgft-snapshot.json       # LC Genre/Form Terms snapshot
│   └── lcmpt-snapshot.json       # LC Medium of Performance snapshot
│
└── tests/                        # Integration tests with sample albums
    ├── fixtures/                  # Test audio files (short clips)
    └── integration/
```

### Dependency Graph

```
tessitura-cli
  ├── tessitura-etl
  │     ├── tessitura-core
  │     └── treadle
  ├── tessitura-graph
  │     └── tessitura-core
  ├── tessitura-search
  │     └── tessitura-core
  └── tessitura-core
```

---

## 10. Development Phases

### Phase 0: Foundation — Treadle Workflow Engine

**Goal**: Build and publish the standalone workflow engine that tessitura's pipeline depends on.

**Status**: ✅ Complete — `treadle` v0.1.0 published to crates.io.

| # | Milestone | Description | Status |
|---|---|---|---|
| 0.1 | Core traits | `WorkItem`, `Stage`, `StageOutcome`, `StageContext`, `StateStore` | ✅ |
| 0.2 | petgraph DAG | `Workflow` struct with builder pattern, dependency edges, cycle detection, topological ordering | ✅ |
| 0.3 | SQLite StateStore | Persistent state: item × stage × subtask → status, with timestamps, attempt counts, error history | ✅ |
| 0.4 | In-memory StateStore | HashMap-based implementation for unit and integration tests | ✅ |
| 0.5 | Executor | Walk items through stages in topological order, respect dependencies, pause at AwaitingReview | ✅ |
| 0.6 | Fan-out | SubTask tracking with per-subtask status, independent retry signaling | ✅ |
| 0.7 | Event stream | tokio broadcast channel: StageStarted, StageCompleted, StageFailed, ReviewRequired, SubTask* | ✅ |
| 0.8 | Pipeline status helpers | Query helpers for "all items at stage X with status Y", visualization-friendly output | ✅ |
| 0.9 | Documentation + publish | README with related projects comparison, crate-level docs, publish to crates.io | ✅ |

---

### Phase 1: Core Data Model + Scan + Identify

**Goal**: Establish the FRBR-rooted schema, scan a music directory, identify recordings via AcoustID/MusicBrainz, and process 3–5 test albums end-to-end through these two stages.

**Deliverable**: `tessitura scan /path/to/music` and `tessitura identify` working against real audio files, with results stored in SQLite.

| # | Milestone | Description | Dependencies |
|---|---|---|---|
| 1.1 | Workspace scaffold | Create the Cargo workspace with all crate stubs, CI (GitHub Actions), rustfmt/clippy config | — |
| 1.2 | tessitura-core: FRBR model | Rust types for Work, Expression (performance), Manifestation (release), Item (file). Serde-derivable. With builder patterns for construction. | — |
| 1.3 | tessitura-core: schema | SQLite schema design and migration system. Tables for works, expressions, manifestations, items, artists, and their relationships. Include provenance table for tracking assertion sources. | 1.2 |
| 1.4 | tessitura-core: taxonomy stubs | Genre, Form, Period, School, Instrumentation types. Initially just the type definitions; controlled vocabulary loading comes in Phase 2. | 1.2 |
| 1.5 | tessitura-etl: scan stage | Implement treadle `Stage` for file scanning. Walk directory tree, extract tags via lofty-rs, compute fingerprints via rusty-chromaprint, create Item records in SQLite. Handle FLAC and MP3 at minimum. | 1.3, treadle |
| 1.6 | tessitura-etl: identify stage | Implement treadle `Stage` for identification. Submit fingerprints to AcoustID API, receive MusicBrainz recording IDs. Fall back to metadata-based matching. Link Items to MB entities in the schema. | 1.5 |
| 1.7 | tessitura-etl: pipeline wiring | Wire scan → identify as a treadle Workflow. Configure SQLite StateStore. Implement basic CLI commands: `tessitura scan`, `tessitura identify`, `tessitura status`. | 1.5, 1.6 |
| 1.8 | Test album validation | Process 3–5 real albums (classical, jazz, electronic) end-to-end. Validate that MB identification succeeds, schema is populated correctly, state tracking works. Document any issues with fingerprinting accuracy. | 1.7 |

**Test Albums** (suggested, covering genre diversity):

- Classical: Bartók String Quartets (Takács Quartet, Decca)
- Jazz: Miles Davis — Kind of Blue
- Electronic: Boards of Canada — Music Has the Right to Children
- Prog: King Crimson — In the Court of the Crimson King
- Contemporary: something from the user's Berklee studies

---

### Phase 2: Enrichment + Harmonization + Review

**Goal**: Build the fan-out enrichment from multiple sources, the mapping/harmonization rules engine, and the human review TUI. Process test albums through the full pipeline.

**Deliverable**: `tessitura enrich`, `tessitura harmonize`, `tessitura review` working with real data. The user can review proposed metadata and approve/edit.

| # | Milestone | Description | Dependencies |
|---|---|---|---|
| 2.1 | tessitura-etl: MusicBrainz enrichment | Fetch full recording, release, and work metadata from MB API. Extract artist relationships, track listings, labels, catalog numbers. Rate-limited (1 req/sec), with backon retry and failsafe circuit breaker. | Phase 1 |
| 2.2 | tessitura-etl: Wikidata enrichment | Query Wikidata for properties linked via MB IDs. Extract: key, form, opus number, instrumentation, period, school/movement. SPARQL queries or REST API. | Phase 1 |
| 2.3 | tessitura-etl: Last.fm enrichment | Fetch folksonomy tags for artist and track. These capture subjective/aesthetic qualities that formal taxonomies miss. | Phase 1 |
| 2.4 | tessitura-etl: LCGFT/LCMPT loading | Ingest LC controlled vocabularies from SKOS/RDF snapshots into the schema. Build hierarchical term relationships (broader/narrower). Map from MB/Wikidata genre strings to LCGFT terms. | 1.4 |
| 2.5 | tessitura-etl: Discogs enrichment | Fetch release-level detail: label, pressing info, personnel credits. Match via MB release ID or metadata search. | Phase 1 |
| 2.6 | tessitura-etl: enrich stage (fan-out) | Implement treadle `Stage` with `StageOutcome::FanOut` for concurrent enrichment from all sources. Each source is a tracked subtask. Per-source retry and circuit breaking. | 2.1–2.5, treadle |
| 2.7 | tessitura-core: mapping rules engine | Configurable rules for normalizing genres, forms, periods, and instrumentation to a controlled vocabulary. Rules stored in TOML. Priority ordering for conflict resolution between sources. | 2.4 |
| 2.8 | tessitura-etl: harmonize stage | Implement treadle `Stage` that applies mapping rules, resolves conflicts, flags ambiguities, and returns `AwaitingReview`. | 2.6, 2.7 |
| 2.9 | tessitura-cli: review TUI | ratatui-based two-level review interface. Album list → track detail drill-down. Show proposed tags, conflicts with resolution rationale, source provenance. Accept all / track-by-track / edit rules / skip. | 2.8 |
| 2.10 | Full pipeline wiring | Wire scan → identify → enrich → harmonize → (review) as a complete treadle Workflow. Test end-to-end with the test albums from Phase 1. | 2.6–2.9 |
| 2.11 | Mapping rules iteration | Process test albums repeatedly, refining mapping rules until the harmonization output is satisfactory for each genre. Document the rule patterns that emerge. | 2.10 |

---

### Phase 3: Graph + Search + Export

**Goal**: Build the music knowledge graph and search capabilities. Enable structured queries ("find all 20th-century string quartets in my collection") and fuzzy/similarity search. Write enriched tags back to audio files.

**Deliverable**: `tessitura search`, `tessitura similar`, `tessitura export` working. The collection is fully searchable.

| # | Milestone | Description | Dependencies |
|---|---|---|---|
| 3.1 | tessitura-graph: schema → graph hydration | Build the petgraph from SQLite on startup. Nodes: works, artists, recordings, genres, forms, instruments, periods, schools. Edges: composed, performed, is_form, is_in_key, belongs_to_school, broader_term, etc. | Phase 2 |
| 3.2 | tessitura-graph: structured queries | Query engine for the graph. Support: "find works by form + period + instrumentation", "find all recordings of a work", "find artists in a school/movement", "show the genre hierarchy under X". Return results as structured tables. | 3.1 |
| 3.3 | tessitura-search: embedding generation | Generate embeddings from combined metadata (genre + form + instrumentation + period as text), Last.fm folksonomy tags, and potentially LCGFT hierarchy position. | Phase 2 |
| 3.4 | tessitura-search: LanceDB indexing | Build and maintain a LanceDB vector index. Support filtered ANN queries (similarity search constrained by structured metadata). | 3.3 |
| 3.5 | tessitura-search: similarity queries | "Find things similar to X" with optional structured filters. Combine vector similarity with graph relationships. | 3.2, 3.4 |
| 3.6 | tessitura-cli: search commands | `tessitura search` for structured queries (graph-backed). `tessitura similar` for fuzzy/associative queries (vector-backed). Table-formatted output. | 3.2, 3.5 |
| 3.7 | tessitura-etl: index stage | Implement treadle `Stage` that updates petgraph and LanceDB after review approval. | 3.1, 3.4 |
| 3.8 | tessitura-etl: export stage | Implement treadle `Stage` that writes enriched metadata back into audio files via lofty-rs. Configurable: which fields to write, how to flatten the rich model into flat tags, Plex-optimized defaults. | Phase 2 |
| 3.9 | End-to-end validation | Full pipeline: scan → identify → enrich → harmonize → review → index → export. Verify that exported tags appear correctly in Plex. Verify graph and vector search return meaningful results. | 3.1–3.8 |

---

### Phase 4: Daemon Mode + Web UI + Plex Integration

**Goal**: Run tessitura as a background service that watches for new files, processes them automatically, and serves a web UI for browsing and review. Deep-link into Plex for playback.

**Deliverable**: `tessitura daemon` running as a persistent service with a web interface.

| # | Milestone | Description | Dependencies |
|---|---|---|---|
| 4.1 | File watcher | Use `notify` crate to watch the music directory for new/changed/removed files. Trigger scan stage automatically. | Phase 3 |
| 4.2 | Background pipeline | Run the ETL pipeline in the background, processing new items through stages. Pause at review for human approval (via web UI). | 4.1, treadle |
| 4.3 | HTTP server (axum) | Serve a REST API: `/api/works`, `/api/recordings`, `/api/artists` (graph queries), `/api/search` (vector similarity), `/api/review` (pending items for approval). | Phase 3 |
| 4.4 | Web UI | Browser-based interface for collection browsing, search, and review workflow. Could be a React/Svelte SPA or server-rendered HTML. | 4.3 |
| 4.5 | Plex integration | Query the Plex local API to map tessitura recordings to Plex item keys. Maintain mapping table: `{recording_id → plex_rating_key}`. Generate deep-link URLs (`https://app.plex.tv/desktop/#!/server/{machineId}/details?key=...`). | 4.3 |
| 4.6 | Plex deep-links in UI | From the web UI, click any recording to play it in Plex. From graph exploration, click through to play. | 4.4, 4.5 |

---

### Phase 5: Advanced Features (Future)

**Goal**: Audio analysis, collaborative features, and ecosystem integration.

| # | Milestone | Description | Dependencies |
|---|---|---|---|
| 5.1 | Audio feature extraction | Extract spectral features (MFCCs, spectral centroid, etc.) from audio files using `rustfft`. Use as additional signal for vector embeddings — enables "sounds like" search based on actual audio, not just metadata. | Phase 3 |
| 5.2 | Streaming service integration | Cross-reference the local collection with Spotify, Apple Music, or Tidal catalogs. "What in my collection is also on Spotify?" or "What's on Spotify that I don't own but might want?" | Phase 3 |
| 5.3 | Study/practice integration | Tag recordings with pedagogical metadata: "good example of sonata form," "demonstrates voice leading in minor keys," "Berklee HM101 reference recording." Support the user's composition studies directly. | Phase 3 |
| 5.4 | Multi-user / shared catalogs | Allow multiple users to contribute to a shared catalog (e.g., a department's reference library). Requires auth, merge strategies, conflict resolution for shared taxonomies. | Phase 4 |
| 5.5 | MCP server | Expose tessitura's catalog as an MCP (Model Context Protocol) server so that Claude or other AI assistants can search the user's music collection, find recordings, and answer musicological questions using the full knowledge graph. | Phase 4 |

---

## 11. Open Questions and Future Directions

### Schema Design

The FRBR model is conceptually clear but the SQLite schema design requires careful thought. Key questions:

- How to represent the many-to-many relationships efficiently (work↔artist, recording↔release, etc.)
- How to store the controlled vocabulary hierarchies (LCGFT broader/narrower terms)
- How to handle the provenance table without it becoming a performance bottleneck
- Whether to use SQLite's JSON1 extension for flexible assertion storage or keep it fully normalized

### Mapping Rules Format

The harmonization rules engine is critical to the user experience. The rules need to be: human-readable and editable (TOML or YAML), composable (rules can reference other rules), ordered (priority for conflict resolution), and testable (you should be able to unit-test a rule against sample input). The exact format and engine design is an open question for Phase 2.

### Vector Embedding Strategy

What text/metadata combination produces the best embeddings for music similarity? This will require experimentation in Phase 3. Candidates: concatenated genre + form + period + instrumentation text, Last.fm folksonomy tag vectors, LCGFT hierarchy position encoding, and eventually audio features.

### Plex Tag Optimization

Which ID3v2/Vorbis comment fields does Plex actually read and display? This needs empirical testing. Known: Plex reads standard genre, artist, album artist, album, track, year. Less clear: how it handles multiple genre tags, whether it reads conductor or composer fields, how it displays catalog numbers.

---

## Appendix A: User's Music Collection Profile

Based on the uploaded file listing, the collection has:

- Well-organized directory structure: Artist/Album/Track
- Mix of FLAC and MP3 files
- Genres represented: classical (significant), jazz, electronic, 70s prog rock, miscellany
- Rough scale: moderate collection (hundreds to low thousands of tracks)

This scale is comfortably within the performance envelope of SQLite + in-memory petgraph + LanceDB for a single-user tool.

## Appendix B: Key External API Rate Limits

| API | Rate Limit | Auth Required | Notes |
|---|---|---|---|
| AcoustID | 3 req/sec | API key (free) | Audio fingerprint lookup |
| MusicBrainz | 1 req/sec | None (User-Agent header) | Be respectful; they're a nonprofit |
| Wikidata | ~5 req/sec | None | SPARQL endpoint; be reasonable |
| Last.fm | 5 req/sec | API key (free) | Folksonomy tags |
| Discogs | 60 req/min unauthenticated, 240 auth'd | Token for higher rate | Release details |

These rate limits inform the backon retry and failsafe circuit breaker configuration per enrichment source.
