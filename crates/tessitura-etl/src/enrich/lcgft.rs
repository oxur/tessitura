//! Vocabulary loaders for LCGFT and LCMPT controlled vocabularies.
//!
//! Loads terms from JSON snapshot files into the database. The snapshot
//! format is a simple JSON array of term objects, not the full JSON-LD
//! format from the Library of Congress.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tessitura_core::schema::Database;
use tessitura_core::taxonomy::{LcgftTerm, LcmptTerm};

/// A vocabulary term as represented in the JSON snapshot files.
#[derive(Debug, Deserialize)]
struct SnapshotTerm {
    uri: String,
    label: String,
    #[serde(default)]
    broader_uri: Option<String>,
    #[serde(default)]
    scope_note: Option<String>,
}

/// Load LCGFT terms from a JSON snapshot file.
///
/// The snapshot file should contain a JSON array of term objects:
/// ```json
/// [
///   {
///     "uri": "http://id.loc.gov/authorities/genreForms/gf2014026639",
///     "label": "String quartets",
///     "broader_uri": "http://id.loc.gov/authorities/genreForms/gf2014026090",
///     "scope_note": "Chamber music for two violins, viola, and cello"
///   }
/// ]
/// ```
///
/// Returns the number of terms loaded.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed, or if database
/// insertion fails.
pub fn load_lcgft(db: &Database, snapshot_path: &Path) -> Result<usize> {
    let content = std::fs::read_to_string(snapshot_path)
        .with_context(|| format!("Failed to read LCGFT snapshot: {}", snapshot_path.display()))?;

    let terms: Vec<SnapshotTerm> = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse LCGFT snapshot: {}",
            snapshot_path.display()
        )
    })?;

    let count = terms.len();

    // Insert terms in broader-first order to satisfy foreign key constraints.
    // First pass: insert terms without broader_uri (top-level terms).
    for term in &terms {
        if term.broader_uri.is_none() {
            let lcgft = LcgftTerm::new(&term.uri, &term.label);
            let lcgft = if let Some(note) = &term.scope_note {
                lcgft.with_scope_note(note)
            } else {
                lcgft
            };
            db.insert_lcgft_term(&lcgft)
                .with_context(|| format!("Failed to insert LCGFT term: {}", term.uri))?;
        }
    }

    // Second pass: insert terms with broader_uri.
    for term in &terms {
        if term.broader_uri.is_some() {
            let mut lcgft = LcgftTerm::new(&term.uri, &term.label);
            if let Some(broader) = &term.broader_uri {
                lcgft = lcgft.with_broader(broader);
            }
            if let Some(note) = &term.scope_note {
                lcgft = lcgft.with_scope_note(note);
            }
            db.insert_lcgft_term(&lcgft)
                .with_context(|| format!("Failed to insert LCGFT term: {}", term.uri))?;
        }
    }

    log::info!(
        "Loaded {} LCGFT terms from {}",
        count,
        snapshot_path.display()
    );
    Ok(count)
}

/// Load LCMPT terms from a JSON snapshot file.
///
/// Same format as LCGFT snapshots. Returns the number of terms loaded.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed, or if database
/// insertion fails.
pub fn load_lcmpt(db: &Database, snapshot_path: &Path) -> Result<usize> {
    let content = std::fs::read_to_string(snapshot_path)
        .with_context(|| format!("Failed to read LCMPT snapshot: {}", snapshot_path.display()))?;

    let terms: Vec<SnapshotTerm> = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse LCMPT snapshot: {}",
            snapshot_path.display()
        )
    })?;

    let count = terms.len();

    // First pass: top-level terms.
    for term in &terms {
        if term.broader_uri.is_none() {
            let lcmpt = LcmptTerm::new(&term.uri, &term.label);
            let lcmpt = if let Some(note) = &term.scope_note {
                lcmpt.with_scope_note(note)
            } else {
                lcmpt
            };
            db.insert_lcmpt_term(&lcmpt)
                .with_context(|| format!("Failed to insert LCMPT term: {}", term.uri))?;
        }
    }

    // Second pass: terms with broader_uri.
    for term in &terms {
        if term.broader_uri.is_some() {
            let mut lcmpt = LcmptTerm::new(&term.uri, &term.label);
            if let Some(broader) = &term.broader_uri {
                lcmpt = lcmpt.with_broader(broader);
            }
            if let Some(note) = &term.scope_note {
                lcmpt = lcmpt.with_scope_note(note);
            }
            db.insert_lcmpt_term(&lcmpt)
                .with_context(|| format!("Failed to insert LCMPT term: {}", term.uri))?;
        }
    }

    log::info!(
        "Loaded {} LCMPT terms from {}",
        count,
        snapshot_path.display()
    );
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_json(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_load_lcgft_empty() {
        let db = Database::open_in_memory().unwrap();
        let file = create_temp_json("[]");
        let count = load_lcgft(&db, file.path()).unwrap();
        assert_eq!(count, 0);
        assert_eq!(db.count_lcgft_terms().unwrap(), 0);
    }

    #[test]
    fn test_load_lcgft_with_terms() {
        let db = Database::open_in_memory().unwrap();
        let json = r#"[
            {
                "uri": "http://example.com/gf-parent",
                "label": "Chamber music"
            },
            {
                "uri": "http://example.com/gf-child",
                "label": "String quartets",
                "broader_uri": "http://example.com/gf-parent",
                "scope_note": "For 2 violins, viola, cello"
            }
        ]"#;
        let file = create_temp_json(json);

        let count = load_lcgft(&db, file.path()).unwrap();
        assert_eq!(count, 2);
        assert_eq!(db.count_lcgft_terms().unwrap(), 2);

        let found = db.get_lcgft_by_label("String quartets").unwrap().unwrap();
        assert_eq!(
            found.broader_uri,
            Some("http://example.com/gf-parent".to_string())
        );
        assert_eq!(
            found.scope_note,
            Some("For 2 violins, viola, cello".to_string())
        );
    }

    #[test]
    fn test_load_lcgft_hierarchy() {
        let db = Database::open_in_memory().unwrap();
        let json = r#"[
            {
                "uri": "http://example.com/gf1",
                "label": "Art music"
            },
            {
                "uri": "http://example.com/gf2",
                "label": "Chamber music",
                "broader_uri": "http://example.com/gf1"
            },
            {
                "uri": "http://example.com/gf3",
                "label": "String quartets",
                "broader_uri": "http://example.com/gf2"
            },
            {
                "uri": "http://example.com/gf4",
                "label": "Piano trios",
                "broader_uri": "http://example.com/gf2"
            }
        ]"#;
        let file = create_temp_json(json);

        load_lcgft(&db, file.path()).unwrap();

        let narrower = db.get_lcgft_narrower("http://example.com/gf2").unwrap();
        assert_eq!(narrower.len(), 2);
        assert_eq!(narrower[0].label, "Piano trios");
        assert_eq!(narrower[1].label, "String quartets");
    }

    #[test]
    fn test_load_lcmpt_with_terms() {
        let db = Database::open_in_memory().unwrap();
        let json = r#"[
            {
                "uri": "http://example.com/mp-strings",
                "label": "bowed strings"
            },
            {
                "uri": "http://example.com/mp-violin",
                "label": "violin",
                "broader_uri": "http://example.com/mp-strings"
            },
            {
                "uri": "http://example.com/mp-viola",
                "label": "viola",
                "broader_uri": "http://example.com/mp-strings"
            }
        ]"#;
        let file = create_temp_json(json);

        let count = load_lcmpt(&db, file.path()).unwrap();
        assert_eq!(count, 3);
        assert_eq!(db.count_lcmpt_terms().unwrap(), 3);

        let narrower = db
            .get_lcmpt_narrower("http://example.com/mp-strings")
            .unwrap();
        assert_eq!(narrower.len(), 2);
    }

    #[test]
    fn test_load_lcgft_invalid_json() {
        let db = Database::open_in_memory().unwrap();
        let file = create_temp_json("not json");
        let result = load_lcgft(&db, file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_lcgft_missing_file() {
        let db = Database::open_in_memory().unwrap();
        let result = load_lcgft(&db, Path::new("/nonexistent/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_lcgft_upsert_replaces() {
        let db = Database::open_in_memory().unwrap();

        // Load initial terms
        let json1 = r#"[{"uri": "http://example.com/gf1", "label": "Original"}]"#;
        let file1 = create_temp_json(json1);
        load_lcgft(&db, file1.path()).unwrap();

        // Load updated terms (same URI, different label)
        let json2 = r#"[{"uri": "http://example.com/gf1", "label": "Updated"}]"#;
        let file2 = create_temp_json(json2);
        load_lcgft(&db, file2.path()).unwrap();

        assert_eq!(db.count_lcgft_terms().unwrap(), 1);
        let found = db.get_lcgft_by_label("Updated").unwrap();
        assert!(found.is_some());
    }
}
