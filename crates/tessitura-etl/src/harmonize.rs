//! Harmonize stage: apply mapping rules to enrichment assertions.
//!
//! Resolves conflicts between multiple sources using source priority,
//! flags ambiguities, and returns `StageOutcome::NeedsReview` so the
//! pipeline pauses for human approval.

use std::path::PathBuf;

use treadle::{Stage, StageContext, StageOutcome};

use tessitura_core::schema::Database;
use tessitura_core::taxonomy::rules::MappingRules;

/// The Harmonize stage: apply mapping rules and resolve conflicts.
///
/// Takes enrichment assertions from the database, applies genre/period/
/// instrument rules, resolves conflicts using source priority, and
/// stores proposed tags in the stage context metadata for review.
#[derive(Debug)]
pub struct HarmonizeStage {
    rules: MappingRules,
    db_path: PathBuf,
}

impl HarmonizeStage {
    /// Create a new `HarmonizeStage` with rules loaded from a TOML file.
    ///
    /// # Errors
    /// Returns an error if the rules file cannot be loaded.
    pub fn new(rules_path: &std::path::Path, db_path: PathBuf) -> Result<Self, String> {
        let rules = MappingRules::load(rules_path).map_err(|e| {
            format!(
                "Failed to load mapping rules from {}: {e}",
                rules_path.display()
            )
        })?;
        Ok(Self { rules, db_path })
    }

    /// Create a `HarmonizeStage` with pre-loaded rules (for testing).
    #[must_use]
    pub fn with_rules(rules: MappingRules, db_path: PathBuf) -> Self {
        Self { rules, db_path }
    }
}

#[async_trait::async_trait]
impl Stage for HarmonizeStage {
    fn name(&self) -> &str {
        "harmonize"
    }

    async fn execute(
        &self,
        item: &dyn treadle::WorkItem,
        ctx: &mut StageContext,
    ) -> treadle::Result<StageOutcome> {
        let db = Database::open(&self.db_path).map_err(|e| {
            treadle::TreadleError::StageExecution(format!("Failed to open database: {e}"))
        })?;

        // 1. Load all assertions for this item
        let assertions = db.get_assertions_for_entity(item.id()).map_err(|e| {
            treadle::TreadleError::StageExecution(format!("Failed to get assertions: {e}"))
        })?;

        if assertions.is_empty() {
            log::info!(
                "No assertions found for {}, skipping harmonization",
                item.id()
            );
            return Ok(StageOutcome::Complete);
        }

        log::info!(
            "Harmonizing {} assertions for {}",
            assertions.len(),
            item.id()
        );

        // 2. Apply genre rules
        let genre_proposals = self.rules.apply_genre_rules(&assertions);

        // 3. Apply period rules (extract composer and year from assertions)
        let composer = assertions
            .iter()
            .find(|a| a.field == "composer")
            .and_then(|a| a.value.as_str())
            .map(String::from);
        #[allow(clippy::cast_possible_truncation)]
        let year = assertions
            .iter()
            .find(|a| a.field == "composed_year" || a.field == "year")
            .and_then(|a| a.value.as_i64())
            .map(|y| y as i32);

        let period_proposal = self.rules.apply_period_rules(composer.as_deref(), year);

        // 4. Apply instrument rules
        let instrument_proposals = self.rules.apply_instrument_rules(&assertions);

        // 5. Collect all proposals
        let mut all_proposals = genre_proposals;
        if let Some(period) = period_proposal {
            all_proposals.push(period);
        }
        all_proposals.extend(instrument_proposals);

        // 6. Store in context metadata for review
        let proposals_json = serde_json::to_value(&all_proposals).map_err(|e| {
            treadle::TreadleError::StageExecution(format!("Failed to serialize proposals: {e}"))
        })?;

        ctx.metadata
            .insert("proposed_tags".to_string(), proposals_json);

        let has_conflicts = all_proposals.iter().any(|p| !p.alternatives.is_empty());

        log::info!(
            "Harmonization complete for {}: {} proposals, {} with conflicts",
            item.id(),
            all_proposals.len(),
            all_proposals
                .iter()
                .filter(|p| !p.alternatives.is_empty())
                .count()
        );

        if has_conflicts {
            // Pause for human review when there are conflicts
            Ok(StageOutcome::NeedsReview)
        } else if all_proposals.is_empty() {
            // Nothing to review
            Ok(StageOutcome::Complete)
        } else {
            // Proposals without conflicts still need review
            Ok(StageOutcome::NeedsReview)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use tessitura_core::provenance::{Assertion, Source};
    use tessitura_core::taxonomy::rules::{GenreRule, MappingRules, PeriodRule};

    fn sample_rules() -> MappingRules {
        let mut source_priority = HashMap::new();
        source_priority.insert("musicbrainz".to_string(), 5);
        source_priority.insert("lastfm".to_string(), 2);
        source_priority.insert("wikidata".to_string(), 6);

        MappingRules {
            source_priority,
            genre_rules: vec![GenreRule {
                name: "classical".to_string(),
                description: None,
                match_any: vec!["classical".to_string()],
                match_source: Vec::new(),
                output_genre: Some("Classical".to_string()),
                output_form: None,
                output_lcgft_label: None,
                confidence: 0.9,
            }],
            period_rules: vec![PeriodRule {
                name: "romantic".to_string(),
                description: None,
                match_composer: vec!["Beethoven".to_string()],
                output_period: "Romantic".to_string(),
                year_range: Some([1800, 1899]),
            }],
            instrument_rules: Vec::new(),
        }
    }

    #[test]
    fn test_harmonize_stage_with_rules() {
        let rules = sample_rules();
        let stage = HarmonizeStage::with_rules(rules, PathBuf::from("/tmp/test.db"));
        assert_eq!(stage.name(), "harmonize");
    }

    #[tokio::test]
    async fn test_harmonize_empty_assertions() {
        let rules = sample_rules();
        let db_dir = tempfile::TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");
        let _db = Database::open(&db_path).unwrap();

        let stage = HarmonizeStage::with_rules(rules, db_path);

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestItem {
            id: String,
        }

        impl treadle::WorkItem for TestItem {
            fn id(&self) -> &str {
                &self.id
            }
        }

        let item = TestItem {
            id: "nonexistent-entity".to_string(),
        };
        let mut ctx = StageContext::new("harmonize".to_string());

        let outcome = stage.execute(&item, &mut ctx).await.unwrap();
        assert_eq!(outcome, StageOutcome::Complete);
    }

    #[tokio::test]
    async fn test_harmonize_with_assertions() {
        let rules = sample_rules();
        let db_dir = tempfile::TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        // Insert some assertions
        let assertion = Assertion::new(
            "test-entity",
            "genre",
            json!("classical music"),
            Source::MusicBrainz,
        )
        .with_confidence(0.9);
        db.insert_assertion(&assertion).unwrap();

        let composer_assertion = Assertion::new(
            "test-entity",
            "composer",
            json!("Beethoven"),
            Source::MusicBrainz,
        )
        .with_confidence(0.95);
        db.insert_assertion(&composer_assertion).unwrap();

        let stage = HarmonizeStage::with_rules(rules, db_path);

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestItem {
            id: String,
        }

        impl treadle::WorkItem for TestItem {
            fn id(&self) -> &str {
                &self.id
            }
        }

        let item = TestItem {
            id: "test-entity".to_string(),
        };
        let mut ctx = StageContext::new("harmonize".to_string());

        let outcome = stage.execute(&item, &mut ctx).await.unwrap();

        // Should need review since there are proposals
        assert_eq!(outcome, StageOutcome::NeedsReview);

        // Check that proposals were stored in metadata
        let proposals = ctx.metadata.get("proposed_tags");
        assert!(proposals.is_some());

        let proposals_array = proposals.unwrap().as_array().unwrap();
        assert!(!proposals_array.is_empty());

        // Should have genre and period proposals
        let fields: Vec<&str> = proposals_array
            .iter()
            .filter_map(|p| p.get("field").and_then(|f| f.as_str()))
            .collect();
        assert!(fields.contains(&"genre"));
        assert!(fields.contains(&"period"));
    }

    #[tokio::test]
    async fn test_harmonize_stores_proposals_in_context() {
        let rules = sample_rules();
        let db_dir = tempfile::TempDir::new().unwrap();
        let db_path = db_dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let assertion = Assertion::new("entity-1", "genre", json!("classical"), Source::LastFm)
            .with_confidence(0.8);
        db.insert_assertion(&assertion).unwrap();

        let stage = HarmonizeStage::with_rules(rules, db_path);

        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        struct TestItem {
            id: String,
        }

        impl treadle::WorkItem for TestItem {
            fn id(&self) -> &str {
                &self.id
            }
        }

        let item = TestItem {
            id: "entity-1".to_string(),
        };
        let mut ctx = StageContext::new("harmonize".to_string());

        stage.execute(&item, &mut ctx).await.unwrap();

        // Verify the proposals are serialized correctly
        let proposals_value = ctx.metadata.get("proposed_tags").unwrap();
        let proposals: Vec<serde_json::Value> =
            serde_json::from_value(proposals_value.clone()).unwrap();
        assert!(!proposals.is_empty());

        // Check the first proposal has expected fields
        let first = &proposals[0];
        assert!(first.get("field").is_some());
        assert!(first.get("value").is_some());
        assert!(first.get("rule_name").is_some());
        assert!(first.get("confidence").is_some());
    }
}
