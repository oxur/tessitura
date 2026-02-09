use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tessitura_core::taxonomy::rules::MappingRules;

/// Get the default rules file path (platform-specific).
fn default_rules_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    Ok(config_dir.join("tessitura").join("taxonomy.toml"))
}

/// Initialize rules file with default content.
pub fn init_rules() -> Result<()> {
    let rules_path = default_rules_path()?;

    if rules_path.exists() {
        println!("✓ Rules file already exists at: {}", rules_path.display());
        return Ok(());
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = rules_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Read default taxonomy.toml from the repo
    // In a packaged binary, we'd embed this with include_str!
    let default_content = if let Ok(content) = fs::read_to_string("config/taxonomy.toml") {
        content
    } else {
        // Fallback: provide minimal default rules
        get_minimal_default_rules()
    };

    fs::write(&rules_path, default_content)?;

    println!("✓ Created default rules file at: {}", rules_path.display());
    println!("\nNext steps:");
    println!("  1. Review the rules: tessitura rules edit");
    println!("  2. Validate syntax: tessitura rules validate");
    println!("  3. Run harmonization: tessitura harmonize");

    Ok(())
}

/// Show the rules file path.
pub fn show_path() -> Result<()> {
    let rules_path = default_rules_path()?;
    println!("{}", rules_path.display());
    Ok(())
}

/// Open rules file in $EDITOR.
pub fn edit_rules() -> Result<()> {
    let rules_path = default_rules_path()?;

    if !rules_path.exists() {
        println!("Rules file not found: {}", rules_path.display());
        println!("\nRun 'tessitura rules init' to create it first.");
        return Ok(());
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "open".to_string()
        } else if cfg!(target_os = "windows") {
            "notepad".to_string()
        } else {
            "vi".to_string()
        }
    });

    Command::new(&editor)
        .arg(&rules_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))?;

    Ok(())
}

/// Validate rules file syntax.
pub fn validate_rules() -> Result<()> {
    let rules_path = default_rules_path()?;

    if !rules_path.exists() {
        println!("Rules file not found: {}", rules_path.display());
        println!("\nRun 'tessitura rules init' to create it first.");
        return Ok(());
    }

    match MappingRules::load(&rules_path) {
        Ok(rules) => {
            println!("✓ Rules file is valid!");
            println!("\nSummary:");
            println!("  Genre rules:       {}", rules.genre_rules.len());
            println!("  Period rules:      {}", rules.period_rules.len());
            println!("  Instrument rules:  {}", rules.instrument_rules.len());
            println!("  Source priorities: {}", rules.source_priority.len());
        }
        Err(e) => {
            println!("✗ Rules file has errors:");
            println!("\n{}", e);
            println!("\nFix the errors and run 'tessitura rules validate' again.");
        }
    }

    Ok(())
}

/// Get minimal default rules as fallback.
fn get_minimal_default_rules() -> String {
    r###"# Tessitura Mapping Rules
#
# This file defines how raw metadata from enrichment sources
# is normalized into controlled vocabulary terms.

[source_priority]
# Higher values win in conflict resolution
embedded_tag = 1
lastfm = 2
discogs = 3
musicbrainz = 5
wikidata = 6
lcgft = 8
user = 10

# Genre/Form Rules
# Match raw metadata values and map to canonical genres/forms

[[genre_rules]]
name = "classical"
match_any = ["classical", "art music"]
output_genre = "Classical"
output_lcgft_label = "Art music"
confidence = 0.9

[[genre_rules]]
name = "jazz"
match_any = ["jazz"]
output_genre = "Jazz"
output_lcgft_label = "Jazz"
confidence = 0.9

# Period Rules
# Infer musical period from composer name or composition year

[[period_rules]]
name = "20th-century"
match_composer = ["Bartók", "Stravinsky", "Schoenberg"]
output_period = "20th Century"
year_range = [1900, 1999]

# Instrument Rules
# Map instrument mentions to standardized terms

[[instrument_rules]]
name = "string-quartet"
match_any = ["string quartet", "2vn va vc"]
output_instruments = ["violin", "violin", "viola", "cello"]
"###
    .to_string()
}
