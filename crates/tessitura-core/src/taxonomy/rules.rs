//! Mapping rules engine for normalizing raw metadata assertions into canonical
//! controlled vocabulary terms.
//!
//! The rules engine loads a TOML configuration file that defines how assertions
//! from various sources (MusicBrainz, Wikidata, Last.fm, Discogs, embedded tags)
//! are matched and transformed into canonical genre, period, and instrumentation
//! values. Source priorities allow higher-authority sources to take precedence
//! when multiple assertions conflict.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use tessitura_core::taxonomy::rules::MappingRules;
//!
//! let rules = MappingRules::load(Path::new("config/taxonomy.toml")).unwrap();
//! let priority = rules.priority_for("musicbrainz");
//! assert!(priority > 0);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};
use crate::provenance::{Assertion, Source};

// ---------------------------------------------------------------------------
// Source name ↔ enum mapping
// ---------------------------------------------------------------------------

/// Canonical string names for each [`Source`] variant, matching the keys used
/// in the TOML configuration file.
const SOURCE_NAMES: &[(Source, &str)] = &[
    (Source::EmbeddedTag, "embedded_tag"),
    (Source::AcoustId, "acoustid"),
    (Source::MusicBrainz, "musicbrainz"),
    (Source::Wikidata, "wikidata"),
    (Source::LastFm, "lastfm"),
    (Source::Lcgft, "lcgft"),
    (Source::Lcmpt, "lcmpt"),
    (Source::Discogs, "discogs"),
    (Source::User, "user"),
];

/// Convert a [`Source`] enum variant to its canonical string name.
fn source_to_str(source: Source) -> &'static str {
    for &(s, name) in SOURCE_NAMES {
        if s == source {
            return name;
        }
    }
    "unknown"
}

/// Convert a string name to a [`Source`] enum variant (case-insensitive).
fn str_to_source(name: &str) -> Option<Source> {
    // Use eq_ignore_ascii_case to avoid allocating a lowercased string
    for &(s, canonical) in SOURCE_NAMES {
        if canonical.eq_ignore_ascii_case(name) {
            return Some(s);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Rule types
// ---------------------------------------------------------------------------

/// Top-level container for mapping rules, loaded from a TOML configuration file.
///
/// The rules engine normalizes raw metadata assertions from multiple sources
/// into canonical controlled vocabulary terms. Source priorities determine
/// which source wins when assertions conflict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRules {
    /// Source name to priority (higher wins).
    #[serde(default)]
    pub source_priority: HashMap<String, u32>,

    /// Rules for mapping genre/style/form/tag assertions to canonical genres.
    #[serde(default)]
    pub genre_rules: Vec<GenreRule>,

    /// Rules for inferring musical period from composer name or year.
    #[serde(default)]
    pub period_rules: Vec<PeriodRule>,

    /// Rules for mapping instrumentation assertions to canonical instruments.
    #[serde(default)]
    pub instrument_rules: Vec<InstrumentRule>,
}

/// A rule for mapping genre, style, form, or tag assertions to canonical values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreRule {
    /// Human-readable rule name for traceability.
    pub name: String,

    /// Optional description of what this rule covers.
    #[serde(default)]
    pub description: Option<String>,

    /// Match if the assertion value contains any of these strings (case-insensitive).
    #[serde(default)]
    pub match_any: Vec<String>,

    /// Only match assertions from these sources. Empty means match all sources.
    #[serde(default)]
    pub match_source: Vec<String>,

    /// Canonical genre to produce (e.g., "Classical > 20th Century").
    #[serde(default)]
    pub output_genre: Option<String>,

    /// Canonical form to produce (e.g., "String quartet").
    #[serde(default)]
    pub output_form: Option<String>,

    /// LCGFT label to link.
    #[serde(default)]
    pub output_lcgft_label: Option<String>,

    /// Confidence score for the rule output (0.0 to 1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

/// A rule for inferring musical period from composer name or composition year.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodRule {
    /// Human-readable rule name for traceability.
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Match if the composer name contains any of these strings (case-insensitive).
    #[serde(default)]
    pub match_composer: Vec<String>,

    /// Canonical period to produce (e.g., "Baroque", "Romantic").
    pub output_period: String,

    /// Optional year range `[start, end]` for matching by composition year.
    #[serde(default)]
    pub year_range: Option<[i32; 2]>,
}

/// A rule for mapping instrumentation assertions to canonical instrument names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentRule {
    /// Human-readable rule name for traceability.
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Match if the assertion value contains any of these strings (case-insensitive).
    #[serde(default)]
    pub match_any: Vec<String>,

    /// Canonical instrument names to produce.
    #[serde(default)]
    pub output_instruments: Vec<String>,

    /// LCMPT labels to link.
    #[serde(default)]
    pub output_lcmpt_labels: Vec<String>,
}

/// A proposed metadata value produced by the rules engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedTag {
    /// The metadata field: "genre", "form", "period", or "instrumentation".
    pub field: String,

    /// The canonical value proposed by the rule.
    pub value: String,

    /// Which source's assertion triggered this proposal.
    pub source: Source,

    /// The name of the rule that produced this proposal.
    pub rule_name: String,

    /// Combined confidence score (rule confidence * assertion confidence).
    pub confidence: f64,

    /// Alternative proposals that were also generated (for conflict resolution).
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
}

/// An alternative proposal that conflicted with the primary proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    /// The alternative canonical value.
    pub value: String,

    /// Which source produced this alternative.
    pub source: Source,

    /// Confidence score.
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.8
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl MappingRules {
    /// Load mapping rules from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(Error::Io)?;
        let rules: Self = toml::from_str(&content).map_err(|e| {
            Error::InvalidData(format!(
                "failed to parse mapping rules from {}: {}",
                path.display(),
                e
            ))
        })?;
        Ok(rules)
    }

    /// Get the priority for a given source name.
    ///
    /// Returns 0 if the source is not found in the priority map.
    pub fn priority_for(&self, source: &str) -> u32 {
        // First try exact match (common case if config uses lowercase keys)
        if let Some(&priority) = self.source_priority.get(source) {
            return priority;
        }
        // Fall back to case-insensitive search
        self.source_priority
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(source))
            .map(|(_, &priority)| priority)
            .unwrap_or(0)
    }

    /// Apply genre rules to a set of assertions and produce proposed tags.
    ///
    /// Filters assertions by relevant fields ("genre", "style", "form", "tag"),
    /// then matches each against the configured genre rules in order. When a
    /// rule matches, a [`ProposedTag`] is created with the rule's output values.
    ///
    /// When multiple rules produce proposals for the same output field and value,
    /// the proposal from the highest-priority source wins, and lower-priority
    /// proposals are recorded as alternatives.
    pub fn apply_genre_rules(&self, assertions: &[Assertion]) -> Vec<ProposedTag> {
        let relevant_fields = ["genre", "style", "form", "tag"];

        let relevant: Vec<&Assertion> = assertions
            .iter()
            .filter(|a| relevant_fields.contains(&a.field.as_str()))
            .collect();

        let mut raw_proposals: Vec<ProposedTag> = Vec::new();

        for assertion in &relevant {
            let assertion_value = assertion_value_as_str(assertion);
            let assertion_lower = assertion_value.to_lowercase();
            let source_name = source_to_str(assertion.source);

            for rule in &self.genre_rules {
                if !rule_matches_source(rule.match_source.as_slice(), source_name) {
                    continue;
                }

                if !rule_matches_value(rule.match_any.as_slice(), &assertion_lower) {
                    continue;
                }

                let assertion_confidence = assertion.confidence.unwrap_or(1.0);
                let combined_confidence = rule.confidence * assertion_confidence;

                // Produce a proposal for each non-None output field.
                if let Some(ref genre) = rule.output_genre {
                    raw_proposals.push(ProposedTag {
                        field: "genre".to_string(),
                        value: genre.clone(),
                        source: assertion.source,
                        rule_name: rule.name.clone(),
                        confidence: combined_confidence,
                        alternatives: Vec::new(),
                    });
                }

                if let Some(ref form) = rule.output_form {
                    raw_proposals.push(ProposedTag {
                        field: "form".to_string(),
                        value: form.clone(),
                        source: assertion.source,
                        rule_name: rule.name.clone(),
                        confidence: combined_confidence,
                        alternatives: Vec::new(),
                    });
                }

                if let Some(ref lcgft) = rule.output_lcgft_label {
                    raw_proposals.push(ProposedTag {
                        field: "genre".to_string(),
                        value: lcgft.clone(),
                        source: assertion.source,
                        rule_name: rule.name.clone(),
                        confidence: combined_confidence,
                        alternatives: Vec::new(),
                    });
                }
            }
        }

        deduplicate_proposals(&self.source_priority, &mut raw_proposals);
        raw_proposals
    }

    /// Infer the musical period from a composer name and/or composition year.
    ///
    /// Tries composer name matching first (case-insensitive substring), then
    /// falls back to year range matching. Returns the first matching rule's
    /// proposal.
    pub fn apply_period_rules(
        &self,
        composer: Option<&str>,
        year: Option<i32>,
    ) -> Option<ProposedTag> {
        // Lowercase once instead of repeatedly in the loop
        let composer_lower = composer.map(str::to_lowercase);

        // First pass: try composer name matching.
        if let Some(ref cl) = composer_lower {
            for rule in &self.period_rules {
                if rule.match_composer.is_empty() {
                    continue;
                }
                // Check if any pattern matches
                for pattern in &rule.match_composer {
                    let pattern_lower = pattern.to_lowercase();
                    if cl.contains(&pattern_lower) {
                        return Some(ProposedTag {
                            field: "period".to_string(),
                            value: rule.output_period.clone(),
                            source: Source::Wikidata,
                            rule_name: rule.name.clone(),
                            confidence: 0.9,
                            alternatives: Vec::new(),
                        });
                    }
                }
            }
        }

        // Second pass: try year range matching.
        if let Some(y) = year {
            for rule in &self.period_rules {
                if let Some([start, end]) = rule.year_range {
                    if y >= start && y <= end {
                        return Some(ProposedTag {
                            field: "period".to_string(),
                            value: rule.output_period.clone(),
                            source: Source::Wikidata,
                            rule_name: rule.name.clone(),
                            confidence: 0.7,
                            alternatives: Vec::new(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Apply instrument rules to a set of assertions and produce proposed tags.
    ///
    /// Filters assertions by the "instrumentation" field, then matches each
    /// against the configured instrument rules.
    pub fn apply_instrument_rules(&self, assertions: &[Assertion]) -> Vec<ProposedTag> {
        let relevant: Vec<&Assertion> = assertions
            .iter()
            .filter(|a| {
                a.field == "instrumentation" || a.field == "instrument" || a.field == "ensemble"
            })
            .collect();

        let mut raw_proposals: Vec<ProposedTag> = Vec::new();

        for assertion in &relevant {
            let assertion_value = assertion_value_as_str(assertion);
            let assertion_lower = assertion_value.to_lowercase();

            for rule in &self.instrument_rules {
                if !rule_matches_value(rule.match_any.as_slice(), &assertion_lower) {
                    continue;
                }

                let assertion_confidence = assertion.confidence.unwrap_or(1.0);
                let combined_confidence = 0.8 * assertion_confidence;

                for instrument in &rule.output_instruments {
                    raw_proposals.push(ProposedTag {
                        field: "instrumentation".to_string(),
                        value: instrument.clone(),
                        source: assertion.source,
                        rule_name: rule.name.clone(),
                        confidence: combined_confidence,
                        alternatives: Vec::new(),
                    });
                }
            }
        }

        deduplicate_proposals(&self.source_priority, &mut raw_proposals);
        raw_proposals
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Extract the assertion value as a string, handling both JSON strings and
/// other JSON value types by converting to their display representation.
///
/// Returns a `Cow` to avoid cloning when the value is already a string.
fn assertion_value_as_str(assertion: &Assertion) -> std::borrow::Cow<'_, str> {
    match &assertion.value {
        serde_json::Value::String(s) => std::borrow::Cow::Borrowed(s),
        other => std::borrow::Cow::Owned(other.to_string()),
    }
}

/// Check if the assertion's source is allowed by the rule's `match_source` list.
/// An empty `match_source` list means all sources are allowed.
///
/// This function assumes both `match_source` and `source_name` are already lowercase.
fn rule_matches_source(match_source: &[String], source_name: &str) -> bool {
    if match_source.is_empty() {
        return true;
    }
    // Assume source_name is already lowercase (from source_to_str)
    match_source.iter().any(|allowed| {
        // Compare case-insensitively without allocating
        allowed.eq_ignore_ascii_case(source_name)
    })
}

/// Check if any of the `match_any` patterns is a substring of the assertion value.
///
/// Assumes `assertion_lower` is already lowercased for efficiency.
fn rule_matches_value(match_any: &[String], assertion_lower: &str) -> bool {
    if match_any.is_empty() {
        return false;
    }
    // assertion_lower is already lowercase, patterns are not.
    // Use eq_ignore_ascii_case for substring check to avoid allocating.
    match_any.iter().any(|pattern| {
        // For case-insensitive substring matching, we need to lowercase the pattern
        // but we can avoid allocating by checking if assertion contains pattern insensitively
        let pattern_lower = pattern.to_lowercase();
        assertion_lower.contains(&pattern_lower)
    })
}

/// Deduplicate proposals by (field, value), keeping the highest-priority source
/// and recording others as alternatives.
fn deduplicate_proposals(source_priority: &HashMap<String, u32>, proposals: &mut Vec<ProposedTag>) {
    use std::collections::hash_map::Entry;

    // Group by (field, value) without cloning the strings for the key
    let mut groups: HashMap<(String, String), Vec<ProposedTag>> = HashMap::new();
    for p in proposals.drain(..) {
        // Only clone when inserting a new entry
        match groups.entry((p.field.clone(), p.value.clone())) {
            Entry::Occupied(mut e) => {
                e.get_mut().push(p);
            }
            Entry::Vacant(e) => {
                e.insert(vec![p]);
            }
        }
    }

    // Pre-compute lowercased source names to avoid repeated allocations
    let get_priority = |source: Source| -> u32 {
        let source_name = source_to_str(source);
        source_priority
            .get(source_name)
            .or_else(|| source_priority.get(&source_name.to_lowercase()))
            .copied()
            .unwrap_or(0)
    };

    for mut group in groups.into_values() {
        // Sort by priority descending, then confidence descending
        group.sort_by(|a, b| {
            let pa = get_priority(a.source);
            let pb = get_priority(b.source);
            pb.cmp(&pa).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        let mut winner = group.remove(0);
        for alt in group {
            winner.alternatives.push(Alternative {
                value: alt.value,
                source: alt.source,
                confidence: alt.confidence,
            });
        }
        proposals.push(winner);
    }
}

// ---------------------------------------------------------------------------
// Public utilities
// ---------------------------------------------------------------------------

/// Convert a [`Source`] to its canonical string name (public re-export).
pub fn source_name(source: Source) -> &'static str {
    source_to_str(source)
}

/// Parse a string into a [`Source`] variant (public re-export).
pub fn parse_source(name: &str) -> Option<Source> {
    str_to_source(name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{Assertion, Source};
    use serde_json::json;
    use std::io::Write;

    /// Helper to create a minimal assertion.
    fn make_assertion(field: &str, value: &str, source: Source) -> Assertion {
        Assertion::new("test-entity", field, json!(value), source)
    }

    /// Helper to create an assertion with confidence.
    fn make_assertion_with_confidence(
        field: &str,
        value: &str,
        source: Source,
        confidence: f64,
    ) -> Assertion {
        Assertion::new("test-entity", field, json!(value), source).with_confidence(confidence)
    }

    /// Helper to build a minimal MappingRules for testing.
    fn sample_rules() -> MappingRules {
        MappingRules {
            source_priority: HashMap::from([
                ("embedded_tag".to_string(), 1),
                ("lastfm".to_string(), 2),
                ("discogs".to_string(), 3),
                ("musicbrainz".to_string(), 5),
                ("wikidata".to_string(), 6),
                ("lcgft".to_string(), 8),
                ("user".to_string(), 10),
            ]),
            genre_rules: vec![
                GenreRule {
                    name: "classical-general".to_string(),
                    description: Some("Match general classical music tags".to_string()),
                    match_any: vec!["classical".to_string()],
                    match_source: vec![],
                    output_genre: Some("Classical".to_string()),
                    output_form: None,
                    output_lcgft_label: None,
                    confidence: 0.8,
                },
                GenreRule {
                    name: "jazz-general".to_string(),
                    description: None,
                    match_any: vec!["jazz".to_string()],
                    match_source: vec![],
                    output_genre: Some("Jazz".to_string()),
                    output_form: None,
                    output_lcgft_label: None,
                    confidence: 0.8,
                },
                GenreRule {
                    name: "string-quartet-form".to_string(),
                    description: None,
                    match_any: vec!["string quartet".to_string()],
                    match_source: vec!["musicbrainz".to_string()],
                    output_genre: None,
                    output_form: Some("String quartet".to_string()),
                    output_lcgft_label: Some("String quartets".to_string()),
                    confidence: 0.9,
                },
            ],
            period_rules: vec![
                PeriodRule {
                    name: "baroque-composers".to_string(),
                    description: Some("Match Baroque-era composers".to_string()),
                    match_composer: vec!["bach".to_string(), "vivaldi".to_string()],
                    output_period: "Baroque".to_string(),
                    year_range: Some([1600, 1750]),
                },
                PeriodRule {
                    name: "romantic-composers".to_string(),
                    description: None,
                    match_composer: vec!["chopin".to_string(), "liszt".to_string()],
                    output_period: "Romantic".to_string(),
                    year_range: Some([1800, 1910]),
                },
                PeriodRule {
                    name: "classical-period".to_string(),
                    description: None,
                    match_composer: vec!["mozart".to_string(), "haydn".to_string()],
                    output_period: "Classical".to_string(),
                    year_range: Some([1750, 1820]),
                },
            ],
            instrument_rules: vec![InstrumentRule {
                name: "string-quartet-instruments".to_string(),
                description: None,
                match_any: vec!["string quartet".to_string()],
                output_instruments: vec![
                    "Violin".to_string(),
                    "Viola".to_string(),
                    "Cello".to_string(),
                ],
                output_lcmpt_labels: vec![
                    "violin".to_string(),
                    "viola".to_string(),
                    "violoncello".to_string(),
                ],
            }],
        }
    }

    // -----------------------------------------------------------------------
    // TOML loading tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_from_toml_basic() {
        let toml_content = r#"
[source_priority]
embedded_tag = 1
musicbrainz = 5
user = 10

[[genre_rules]]
name = "classical"
match_any = ["classical"]
confidence = 0.8

[[period_rules]]
name = "baroque"
match_composer = ["bach"]
output_period = "Baroque"
year_range = [1600, 1750]

[[instrument_rules]]
name = "piano"
match_any = ["piano"]
output_instruments = ["Piano"]
output_lcmpt_labels = ["piano"]
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_rules.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        let rules = MappingRules::load(&path).unwrap();
        assert_eq!(rules.source_priority.len(), 3);
        assert_eq!(rules.genre_rules.len(), 1);
        assert_eq!(rules.genre_rules[0].name, "classical");
        assert_eq!(rules.period_rules.len(), 1);
        assert_eq!(rules.period_rules[0].output_period, "Baroque");
        assert_eq!(rules.instrument_rules.len(), 1);
        assert_eq!(rules.instrument_rules[0].output_instruments, vec!["Piano"]);
    }

    #[test]
    fn test_load_from_toml_with_optional_fields() {
        let toml_content = r#"
[[genre_rules]]
name = "jazz-mb-only"
description = "Jazz from MusicBrainz only"
match_any = ["jazz"]
match_source = ["musicbrainz"]
output_genre = "Jazz"
output_form = "Jazz composition"
output_lcgft_label = "Jazz"
confidence = 0.85
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_optional.toml");
        std::fs::write(&path, toml_content).unwrap();

        let rules = MappingRules::load(&path).unwrap();
        let rule = &rules.genre_rules[0];
        assert_eq!(
            rule.description.as_deref(),
            Some("Jazz from MusicBrainz only")
        );
        assert_eq!(rule.match_source, vec!["musicbrainz"]);
        assert_eq!(rule.output_genre.as_deref(), Some("Jazz"));
        assert_eq!(rule.output_form.as_deref(), Some("Jazz composition"));
        assert_eq!(rule.output_lcgft_label.as_deref(), Some("Jazz"));
        assert!((rule.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_load_from_toml_defaults() {
        let toml_content = r#"
[[genre_rules]]
name = "minimal"
match_any = ["rock"]
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_defaults.toml");
        std::fs::write(&path, toml_content).unwrap();

        let rules = MappingRules::load(&path).unwrap();
        let rule = &rules.genre_rules[0];
        assert!(rule.description.is_none());
        assert!(rule.match_source.is_empty());
        assert!(rule.output_genre.is_none());
        assert!(rule.output_form.is_none());
        assert!(rule.output_lcgft_label.is_none());
        assert!((rule.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_load_from_toml_nonexistent_file() {
        let result = MappingRules::load(Path::new("/nonexistent/path/rules.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_toml_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not valid toml [[[[").unwrap();

        let result = MappingRules::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.toml");
        std::fs::write(&path, "").unwrap();

        let rules = MappingRules::load(&path).unwrap();
        assert!(rules.source_priority.is_empty());
        assert!(rules.genre_rules.is_empty());
        assert!(rules.period_rules.is_empty());
        assert!(rules.instrument_rules.is_empty());
    }

    // -----------------------------------------------------------------------
    // Priority tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_priority_for_known_source() {
        let rules = sample_rules();
        assert_eq!(rules.priority_for("musicbrainz"), 5);
        assert_eq!(rules.priority_for("user"), 10);
        assert_eq!(rules.priority_for("embedded_tag"), 1);
    }

    #[test]
    fn test_priority_for_case_insensitive() {
        let rules = sample_rules();
        assert_eq!(rules.priority_for("MusicBrainz"), 5);
        assert_eq!(rules.priority_for("MUSICBRAINZ"), 5);
        assert_eq!(rules.priority_for("User"), 10);
    }

    #[test]
    fn test_priority_for_unknown_source() {
        let rules = sample_rules();
        assert_eq!(rules.priority_for("spotify"), 0);
        assert_eq!(rules.priority_for(""), 0);
    }

    // -----------------------------------------------------------------------
    // Genre rule matching tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_genre_rule_exact_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("genre", "classical", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(!proposals.is_empty());
        let genre_proposals: Vec<&ProposedTag> =
            proposals.iter().filter(|p| p.field == "genre").collect();
        assert!(genre_proposals.iter().any(|p| p.value == "Classical"));
    }

    #[test]
    fn test_genre_rule_substring_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "genre",
            "20th century classical",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Classical"));
    }

    #[test]
    fn test_genre_rule_case_insensitive_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("genre", "CLASSICAL", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Classical"));
    }

    #[test]
    fn test_genre_rule_case_insensitive_pattern() {
        // Ensure the pattern itself is compared case-insensitively.
        let mut rules = sample_rules();
        rules.genre_rules[0].match_any = vec!["CLASSICAL".to_string()];

        let assertions = vec![make_assertion(
            "genre",
            "classical music",
            Source::MusicBrainz,
        )];
        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Classical"));
    }

    #[test]
    fn test_genre_rule_no_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("genre", "metal", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_genre_rule_irrelevant_field_ignored() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("key", "C major", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_genre_rule_matches_style_field() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("style", "classical", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Classical"));
    }

    #[test]
    fn test_genre_rule_matches_tag_field() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("tag", "jazz fusion", Source::LastFm)];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Jazz"));
    }

    #[test]
    fn test_genre_rule_matches_form_field() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "form",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals
            .iter()
            .any(|p| p.value == "String quartet" && p.field == "form"));
    }

    #[test]
    fn test_genre_rule_source_filtering_allows_matching_source() {
        let rules = sample_rules();
        // The "string-quartet-form" rule only matches from musicbrainz.
        let assertions = vec![make_assertion(
            "genre",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals
            .iter()
            .any(|p| p.field == "form" && p.value == "String quartet"));
    }

    #[test]
    fn test_genre_rule_source_filtering_rejects_other_source() {
        let rules = sample_rules();
        // The "string-quartet-form" rule only matches from musicbrainz,
        // so a LastFm assertion should not produce form/lcgft proposals.
        let assertions = vec![make_assertion("genre", "string quartet", Source::LastFm)];

        let proposals = rules.apply_genre_rules(&assertions);
        // Should NOT have the form proposal from the source-restricted rule.
        assert!(!proposals
            .iter()
            .any(|p| p.field == "form" && p.value == "String quartet"));
    }

    #[test]
    fn test_genre_rule_confidence_combination() {
        let rules = sample_rules();
        let assertions = vec![make_assertion_with_confidence(
            "genre",
            "classical",
            Source::MusicBrainz,
            0.9,
        )];

        let proposals = rules.apply_genre_rules(&assertions);
        let classical = proposals.iter().find(|p| p.value == "Classical").unwrap();
        // Rule confidence is 0.8, assertion confidence is 0.9 => 0.72
        assert!((classical.confidence - 0.72).abs() < f64::EPSILON);
    }

    #[test]
    fn test_genre_rule_default_assertion_confidence() {
        let rules = sample_rules();
        // No explicit confidence on the assertion (None).
        let assertions = vec![make_assertion("genre", "classical", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        let classical = proposals.iter().find(|p| p.value == "Classical").unwrap();
        // Rule confidence is 0.8, assertion confidence defaults to 1.0 => 0.8
        assert!((classical.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_genre_rule_multiple_matches() {
        let rules = sample_rules();
        let assertions = vec![
            make_assertion("genre", "classical", Source::MusicBrainz),
            make_assertion("tag", "jazz", Source::LastFm),
        ];

        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.iter().any(|p| p.value == "Classical"));
        assert!(proposals.iter().any(|p| p.value == "Jazz"));
    }

    #[test]
    fn test_genre_rule_empty_assertions() {
        let rules = sample_rules();
        let proposals = rules.apply_genre_rules(&[]);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_genre_rule_empty_match_any_never_matches() {
        let mut rules = sample_rules();
        rules.genre_rules = vec![GenreRule {
            name: "empty-patterns".to_string(),
            description: None,
            match_any: vec![],
            match_source: vec![],
            output_genre: Some("Should not match".to_string()),
            output_form: None,
            output_lcgft_label: None,
            confidence: 0.8,
        }];

        let assertions = vec![make_assertion("genre", "anything", Source::MusicBrainz)];
        let proposals = rules.apply_genre_rules(&assertions);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_genre_rule_deduplication_keeps_highest_priority() {
        let rules = sample_rules();
        // Both sources produce "Classical" - musicbrainz (priority 5) should win
        // over lastfm (priority 2).
        let assertions = vec![
            make_assertion("genre", "classical", Source::LastFm),
            make_assertion("genre", "classical", Source::MusicBrainz),
        ];

        let proposals = rules.apply_genre_rules(&assertions);
        let classical: Vec<&ProposedTag> = proposals
            .iter()
            .filter(|p| p.value == "Classical")
            .collect();
        assert_eq!(classical.len(), 1);
        assert_eq!(classical[0].source, Source::MusicBrainz);
        assert_eq!(classical[0].alternatives.len(), 1);
        assert_eq!(classical[0].alternatives[0].source, Source::LastFm);
    }

    #[test]
    fn test_genre_rule_json_non_string_value() {
        let rules = sample_rules();
        // Test that non-string JSON values are handled gracefully.
        let assertion = Assertion::new("test-entity", "genre", json!(42), Source::MusicBrainz);
        let proposals = rules.apply_genre_rules(&[assertion]);
        // "42" should not match any rule.
        assert!(proposals.is_empty());
    }

    // -----------------------------------------------------------------------
    // Period rule matching tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_period_rule_match_by_composer_name() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("Johann Sebastian Bach"), None);
        assert!(result.is_some());
        let tag = result.unwrap();
        assert_eq!(tag.value, "Baroque");
        assert_eq!(tag.field, "period");
    }

    #[test]
    fn test_period_rule_composer_case_insensitive() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("BACH"), None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Baroque");
    }

    #[test]
    fn test_period_rule_composer_substring_match() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("Frederic Chopin"), None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Romantic");
    }

    #[test]
    fn test_period_rule_match_by_year_within_range() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(None, Some(1700));
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Baroque");
    }

    #[test]
    fn test_period_rule_match_by_year_boundary_start() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(None, Some(1600));
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Baroque");
    }

    #[test]
    fn test_period_rule_match_by_year_boundary_end() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(None, Some(1750));
        assert!(result.is_some());
        // 1750 falls in both Baroque [1600,1750] and Classical [1750,1820].
        // First match wins, so Baroque.
        assert_eq!(result.unwrap().value, "Baroque");
    }

    #[test]
    fn test_period_rule_no_match() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("Unknown Composer"), Some(1400));
        assert!(result.is_none());
    }

    #[test]
    fn test_period_rule_composer_takes_priority_over_year() {
        let rules = sample_rules();
        // Mozart (Classical period) but year 1850 (Romantic range).
        // Composer match should win.
        let result = rules.apply_period_rules(Some("Mozart"), Some(1850));
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Classical");
    }

    #[test]
    fn test_period_rule_none_inputs() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(None, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_period_rule_confidence_from_composer() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("Vivaldi"), None);
        assert!(result.is_some());
        assert!((result.unwrap().confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_period_rule_confidence_from_year() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(None, Some(1850));
        assert!(result.is_some());
        assert!((result.unwrap().confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_period_rule_second_composer_in_list() {
        let rules = sample_rules();
        let result = rules.apply_period_rules(Some("Antonio Vivaldi"), None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Baroque");
    }

    // -----------------------------------------------------------------------
    // Instrument rule matching tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_instrument_rule_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "instrumentation",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(!proposals.is_empty());
        let values: Vec<&str> = proposals.iter().map(|p| p.value.as_str()).collect();
        assert!(values.contains(&"Violin"));
        assert!(values.contains(&"Viola"));
        assert!(values.contains(&"Cello"));
    }

    #[test]
    fn test_instrument_rule_case_insensitive() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "instrumentation",
            "STRING QUARTET",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_no_match() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "instrumentation",
            "brass quintet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_irrelevant_field_ignored() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "genre",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_matches_instrument_field() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "instrument",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_matches_ensemble_field() {
        let rules = sample_rules();
        let assertions = vec![make_assertion(
            "ensemble",
            "string quartet",
            Source::MusicBrainz,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        assert!(!proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_empty_assertions() {
        let rules = sample_rules();
        let proposals = rules.apply_instrument_rules(&[]);
        assert!(proposals.is_empty());
    }

    #[test]
    fn test_instrument_rule_confidence_combination() {
        let rules = sample_rules();
        let assertions = vec![make_assertion_with_confidence(
            "instrumentation",
            "string quartet",
            Source::MusicBrainz,
            0.9,
        )];

        let proposals = rules.apply_instrument_rules(&assertions);
        // Instrument rule confidence is 0.8, assertion is 0.9 => 0.72
        for p in &proposals {
            assert!((p.confidence - 0.72).abs() < f64::EPSILON);
        }
    }

    // -----------------------------------------------------------------------
    // Source name conversion tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_source_to_str_all_variants() {
        assert_eq!(source_to_str(Source::EmbeddedTag), "embedded_tag");
        assert_eq!(source_to_str(Source::AcoustId), "acoustid");
        assert_eq!(source_to_str(Source::MusicBrainz), "musicbrainz");
        assert_eq!(source_to_str(Source::Wikidata), "wikidata");
        assert_eq!(source_to_str(Source::LastFm), "lastfm");
        assert_eq!(source_to_str(Source::Lcgft), "lcgft");
        assert_eq!(source_to_str(Source::Lcmpt), "lcmpt");
        assert_eq!(source_to_str(Source::Discogs), "discogs");
        assert_eq!(source_to_str(Source::User), "user");
    }

    #[test]
    fn test_str_to_source_all_variants() {
        assert_eq!(str_to_source("embedded_tag"), Some(Source::EmbeddedTag));
        assert_eq!(str_to_source("acoustid"), Some(Source::AcoustId));
        assert_eq!(str_to_source("musicbrainz"), Some(Source::MusicBrainz));
        assert_eq!(str_to_source("wikidata"), Some(Source::Wikidata));
        assert_eq!(str_to_source("lastfm"), Some(Source::LastFm));
        assert_eq!(str_to_source("lcgft"), Some(Source::Lcgft));
        assert_eq!(str_to_source("lcmpt"), Some(Source::Lcmpt));
        assert_eq!(str_to_source("discogs"), Some(Source::Discogs));
        assert_eq!(str_to_source("user"), Some(Source::User));
    }

    #[test]
    fn test_str_to_source_case_insensitive() {
        assert_eq!(str_to_source("MUSICBRAINZ"), Some(Source::MusicBrainz));
        assert_eq!(str_to_source("MusicBrainz"), Some(Source::MusicBrainz));
    }

    #[test]
    fn test_str_to_source_unknown() {
        assert_eq!(str_to_source("spotify"), None);
        assert_eq!(str_to_source(""), None);
    }

    #[test]
    fn test_source_name_public_api() {
        assert_eq!(source_name(Source::MusicBrainz), "musicbrainz");
    }

    #[test]
    fn test_parse_source_public_api() {
        assert_eq!(parse_source("musicbrainz"), Some(Source::MusicBrainz));
        assert_eq!(parse_source("unknown"), None);
    }

    // -----------------------------------------------------------------------
    // Unmatched assertions
    // -----------------------------------------------------------------------

    #[test]
    fn test_unmatched_assertions_produce_no_proposals() {
        let rules = sample_rules();

        // Genre assertions that don't match any rule.
        let assertions = vec![
            make_assertion("genre", "polka", Source::MusicBrainz),
            make_assertion("style", "ambient drone", Source::LastFm),
            make_assertion("tag", "experimental noise", Source::Discogs),
        ];

        let genre_proposals = rules.apply_genre_rules(&assertions);
        assert!(genre_proposals.is_empty());

        // Instrument assertions that don't match any rule.
        let instrument_assertions = vec![make_assertion(
            "instrumentation",
            "theremin",
            Source::MusicBrainz,
        )];
        let instrument_proposals = rules.apply_instrument_rules(&instrument_assertions);
        assert!(instrument_proposals.is_empty());

        // Period with unknown composer and out-of-range year.
        let period_proposal = rules.apply_period_rules(Some("Unknown Person"), Some(1200));
        assert!(period_proposal.is_none());
    }

    // -----------------------------------------------------------------------
    // ProposedTag field values
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposed_tag_has_correct_rule_name() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("genre", "classical", Source::MusicBrainz)];

        let proposals = rules.apply_genre_rules(&assertions);
        let classical = proposals.iter().find(|p| p.value == "Classical").unwrap();
        assert_eq!(classical.rule_name, "classical-general");
    }

    #[test]
    fn test_proposed_tag_has_correct_source() {
        let rules = sample_rules();
        let assertions = vec![make_assertion("genre", "jazz", Source::Discogs)];

        let proposals = rules.apply_genre_rules(&assertions);
        let jazz = proposals.iter().find(|p| p.value == "Jazz").unwrap();
        assert_eq!(jazz.source, Source::Discogs);
    }

    // -----------------------------------------------------------------------
    // Serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_mapping_rules_serialize_deserialize_roundtrip() {
        let rules = sample_rules();
        let toml_str = toml::to_string(&rules).unwrap();
        let parsed: MappingRules = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.source_priority.len(), rules.source_priority.len());
        assert_eq!(parsed.genre_rules.len(), rules.genre_rules.len());
        assert_eq!(parsed.period_rules.len(), rules.period_rules.len());
        assert_eq!(parsed.instrument_rules.len(), rules.instrument_rules.len());
        assert_eq!(parsed.genre_rules[0].name, rules.genre_rules[0].name);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_genre_rule_with_lcgft_output_only() {
        let mut rules = sample_rules();
        rules.genre_rules = vec![GenreRule {
            name: "lcgft-only".to_string(),
            description: None,
            match_any: vec!["fugue".to_string()],
            match_source: vec![],
            output_genre: None,
            output_form: None,
            output_lcgft_label: Some("Fugues".to_string()),
            confidence: 0.85,
        }];

        let assertions = vec![make_assertion("genre", "fugue", Source::Lcgft)];
        let proposals = rules.apply_genre_rules(&assertions);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].value, "Fugues");
    }

    #[test]
    fn test_period_rule_with_empty_composer_list_skipped_for_composer() {
        // A period rule with no match_composer should only be reachable via year range.
        let mut rules = sample_rules();
        rules.period_rules = vec![PeriodRule {
            name: "year-only-rule".to_string(),
            description: None,
            match_composer: vec![],
            output_period: "Modern".to_string(),
            year_range: Some([1900, 2000]),
        }];

        // Composer name should not match (empty list is skipped).
        let result = rules.apply_period_rules(Some("Stravinsky"), None);
        assert!(result.is_none());

        // But year should match.
        let result = rules.apply_period_rules(None, Some(1950));
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, "Modern");
    }

    #[test]
    fn test_multiple_genre_outputs_from_single_rule() {
        let mut rules = sample_rules();
        rules.genre_rules = vec![GenreRule {
            name: "multi-output".to_string(),
            description: None,
            match_any: vec!["string quartet".to_string()],
            match_source: vec![],
            output_genre: Some("Chamber Music".to_string()),
            output_form: Some("String quartet".to_string()),
            output_lcgft_label: Some("String quartets".to_string()),
            confidence: 0.9,
        }];

        let assertions = vec![make_assertion(
            "genre",
            "string quartet",
            Source::MusicBrainz,
        )];
        let proposals = rules.apply_genre_rules(&assertions);
        // Should have at least genre + form + lcgft proposals.
        assert!(proposals.len() >= 2);
        assert!(proposals
            .iter()
            .any(|p| p.field == "genre" && p.value == "Chamber Music"));
        assert!(proposals
            .iter()
            .any(|p| p.field == "form" && p.value == "String quartet"));
        // Note: lcgft output also goes to "genre" field, may be deduplicated
        // with "Chamber Music" or kept separately depending on value.
    }

    #[test]
    fn test_helper_assertion_value_as_str() {
        let string_assertion = Assertion::new(
            "test-entity",
            "genre",
            json!("Classical"),
            Source::MusicBrainz,
        );
        assert_eq!(assertion_value_as_str(&string_assertion), "Classical");

        let number_assertion = Assertion::new("test-entity", "year", json!(1750), Source::Wikidata);
        assert_eq!(assertion_value_as_str(&number_assertion), "1750");

        let array_assertion =
            Assertion::new("test-entity", "tags", json!(["a", "b"]), Source::LastFm);
        assert_eq!(assertion_value_as_str(&array_assertion), "[\"a\",\"b\"]");
    }

    #[test]
    fn test_helper_rule_matches_source_empty_allows_all() {
        assert!(rule_matches_source(&[], "musicbrainz"));
        assert!(rule_matches_source(&[], "anything"));
    }

    #[test]
    fn test_helper_rule_matches_source_filters() {
        let sources = vec!["musicbrainz".to_string(), "wikidata".to_string()];
        assert!(rule_matches_source(&sources, "musicbrainz"));
        assert!(rule_matches_source(&sources, "wikidata"));
        assert!(!rule_matches_source(&sources, "lastfm"));
    }

    #[test]
    fn test_helper_rule_matches_value_empty_never_matches() {
        assert!(!rule_matches_value(&[], "anything"));
    }

    #[test]
    fn test_helper_rule_matches_value_substring() {
        let patterns = vec!["classical".to_string()];
        assert!(rule_matches_value(
            &patterns,
            "20th century classical music"
        ));
        assert!(!rule_matches_value(&patterns, "jazz"));
    }
}
