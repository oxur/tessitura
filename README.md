# Tessitura

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]
[![][license badge]][license] [![][status badge]][status]

[![][logo]][logo-large]

*A musicological audio media library cataloging tool for serious musicians, written in Rust.*

The name comes from *tessitura* — the vocal or instrumental range where a
performer is most comfortable and expressive. The metaphor: finding the range
where your music collection is most naturally discoverable and useful.

## About

Tessitura is a CLI-first tool for cataloging, enriching, and searching a
personal digital music library with musicological rigor. It is designed for
serious musicians, composition students, musicologists, and archivists — people
for whom "Genre: Classical" is not remotely sufficient.

Tessitura is **not** a music player or streaming service. It is a cataloging,
enrichment, and search tool that can optionally write enriched metadata back
into audio files (FLAC, MP3) for consumption by other tools.

## The Problem

No consumer audio tagging standard can express the full relational model of
music metadata. ID3v2 and Vorbis comments give you flat key-value pairs.
There's no native way to express:

> "This is a performance of Bartók's String Quartet No. 4, III. Non troppo
> lento, performed by the Takács Quartet, in a 1998 Decca recording, which I
> own as a FLAC rip."

Tessitura solves this by maintaining a full relational model internally (rooted
in the FRBR library science hierarchy) and projecting a flattened version into
audio tags when needed.

| FRBR Level | Music Example |
|---|---|
| **Work** | Beethoven's Symphony No. 5 |
| **Expression** | Karajan/BPO 1962 interpretation |
| **Manifestation** | The specific DG LP release |
| **Item** | Your FLAC rip of that release |

## Usage

```sh
# Scan a music directory
tessitura scan /path/to/music

# Identify recordings via AcoustID/MusicBrainz
tessitura identify

# Enrich metadata from multiple sources
tessitura enrich

# Review proposed metadata (launches TUI)
tessitura review

# Search your collection
tessitura search --form "string quartet" --period "20th century" --key "a minor"

# Find similar recordings
tessitura similar "Bartók String Quartet No. 4"

# Check pipeline status
tessitura status "String Quartets 1-6"

# Write enriched tags back to audio files
tessitura export
```

## Architecture

```
CLI / TUI Interface
        │
   ┌────┴────┐
   │         │
petgraph  LanceDB         ← structured + vector search
   │         │
   └────┬────┘
        │
    SQLite (FRBR schema)   ← source of truth
        │
  ETL Pipeline (tessitura)   ← scan → identify → enrich → harmonize → review → index → export
```

Metadata is enriched from multiple sources — MusicBrainz, Wikidata, Last.fm,
Library of Congress controlled vocabularies (LCGFT/LCMPT), and Discogs — with
full provenance tracking per assertion.

## Project Structure

```
tessitura/
├── crates/
│   ├── tessitura-core/     # FRBR domain model, schema, taxonomy
│   ├── tessitura-etl/      # Pipeline stages (scan, identify, enrich, ...)
│   ├── tessitura-graph/    # petgraph knowledge graph + structured queries
│   ├── tessitura-search/   # LanceDB vector search
│   └── tessitura-cli/      # CLI + TUI entry point
└── config/                 # Mapping rules, vocabulary snapshots
```

The ETL pipeline is powered by [tessitura](https://crates.io/crates/tessitura), a
standalone workflow engine crate with persistent state, resumability, and
human-in-the-loop review gates.

## Status

Pre-development — architecture and design phase complete. See the
[project plan](docs/design/06-final/0001-tessitura-project-plan.md) for full
details.

## License

Apache-2.0/MIT

[//]: ---Named-Links---

[logo]: assets/images/logo/v1-x250.png
[logo-large]: assets/images/logo/v1.png
[build]: https://github.com/oxur/tessitura/actions/workflows/ci.yml
[build-badge]: https://github.com/oxur/tessitura/actions/workflows/ci.yml/badge.svg
[crate]: https://crates.io/crates/tessitura
[crate-badge]: https://img.shields.io/crates/v/tessitura.svg
[docs]: https://docs.rs/tessitura/
[docs-badge]: https://img.shields.io/badge/rust-documentation-blue.svg
[tag-badge]: https://img.shields.io/github/tag/oxur/tessitura.svg
[tag]: https://github.com/oxur/tessitura/tags
[license]: LICENSE-APACHE
[license badge]: https://img.shields.io/badge/License-Apache%202.0%2FMIT-blue.svg
[status]: https://github.com/oxur/tessitura
[status badge]: https://img.shields.io/badge/Status-Pre--development-yellow.svg
