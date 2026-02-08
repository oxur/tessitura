use anyhow::{Context, Result};
use tessitura_etl::{config, Config};
use toml_edit::{DocumentMut, Item, Value};

/// Show the current effective configuration.
pub fn show_config() -> Result<()> {
    let config = Config::load()?;

    println!("Current Configuration");
    println!("=====================\n");

    println!("Config file: {}", config::config_file_path().display());

    let exists = config::config_file_path().exists();
    println!(
        "File exists: {}\n",
        if exists { "yes" } else { "no (using defaults)" }
    );

    println!("Settings:");
    println!(
        "  acoustid_api_key: {}",
        config.acoustid_api_key.as_deref().unwrap_or("<not set>")
    );
    println!("  database_path: {}", config.database_path.display());
    println!("  logging.level: {:?}", config.logging.level());
    println!("  logging.coloured: {}", config.logging.coloured());
    println!("  logging.output: {:?}", config.logging.output());

    println!("\nPriority: CLI args > ENV vars (TESS_*) > Config file > Defaults");

    Ok(())
}

/// Get a specific config value.
pub fn get_config(key: Option<String>) -> Result<()> {
    if let Some(key) = key {
        let config_path = config::config_file_path();

        if !config_path.exists() {
            anyhow::bail!(
                "Config file does not exist: {}\n\nRun 'tessitura config init' to create it.",
                config_path.display()
            );
        }

        let contents =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;
        let doc = contents
            .parse::<DocumentMut>()
            .context("Failed to parse config file")?;

        // Split key by dots for nested access
        let key_parts: Vec<&str> = key.split('.').collect();

        // Navigate to the value
        let mut current: &Item = doc.as_item();
        for part in &key_parts {
            if let Some(table) = current.as_table() {
                if let Some(item) = table.get(*part) {
                    current = item;
                } else {
                    anyhow::bail!("Key not found: {}", key);
                }
            } else {
                anyhow::bail!("Cannot navigate into non-table value at: {}", key);
            }
        }

        // Print the value
        if let Some(value) = current.as_value() {
            println!("{}", format_toml_value(value));
        } else {
            anyhow::bail!("Key '{}' is not a value (it may be a table/section)", key);
        }
    } else {
        // No key provided, show entire config file contents
        let config_path = config::config_file_path();

        if config_path.exists() {
            let contents =
                std::fs::read_to_string(&config_path).context("Failed to read config file")?;
            print!("{}", contents);
        } else {
            println!("Config file does not exist: {}", config_path.display());
            println!("\nRun 'tessitura config init' to create it.");
        }
    }

    Ok(())
}

/// Format a TOML value for display
fn format_toml_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.value().to_string(),
        Value::Integer(i) => i.value().to_string(),
        Value::Float(f) => f.value().to_string(),
        Value::Boolean(b) => b.value().to_string(),
        Value::Datetime(d) => d.value().to_string(),
        Value::Array(a) => a.to_string(),
        Value::InlineTable(t) => t.to_string(),
    }
}

/// Set a config value.
pub fn set_config(key: String, value: String) -> Result<()> {
    let config_path = config::config_file_path();

    // Ensure config file exists
    config::ensure_config_file()?;

    // Read existing config
    let contents = std::fs::read_to_string(&config_path).context("Failed to read config file")?;
    let mut doc = contents
        .parse::<DocumentMut>()
        .context("Failed to parse config file")?;

    // Split key by dots for nested access
    let key_parts: Vec<&str> = key.split('.').collect();

    // Navigate to the parent table, creating sections as needed
    let mut current = doc.as_table_mut();
    for part in &key_parts[..key_parts.len() - 1] {
        // Get or create intermediate table
        if !current.contains_key(*part) {
            current[*part] = Item::Table(toml_edit::Table::new());
        }

        current = current
            .get_mut(*part)
            .and_then(|item| item.as_table_mut())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Path '{}' exists but is not a table",
                    key_parts[..key_parts.len() - 1].join(".")
                )
            })?;
    }

    // Set the final value
    let final_key = key_parts[key_parts.len() - 1];
    let toml_value = infer_toml_value(&value)?;
    current[final_key] = Item::Value(toml_value);

    // Write back
    std::fs::write(&config_path, doc.to_string()).context("Failed to write config file")?;

    println!("✓ Updated {} = {}", key, value);
    println!("  in {}", config_path.display());

    Ok(())
}

/// Infer the TOML value type from a string
fn infer_toml_value(s: &str) -> Result<Value> {
    let trimmed = s.trim();

    // Try inline table (e.g., { fg = "HiBlack", bg = "Reset" })
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Parse as TOML to get the inline table
        let toml_doc = format!("temp = {}", trimmed)
            .parse::<DocumentMut>()
            .context("Failed to parse inline table")?;

        if let Some(value) = toml_doc.get("temp").and_then(|item| item.as_value()) {
            return Ok(value.clone());
        }

        anyhow::bail!("Failed to extract inline table value");
    }

    // Try boolean
    if trimmed == "true" {
        return Ok(Value::Boolean(toml_edit::Formatted::new(true)));
    }
    if trimmed == "false" {
        return Ok(Value::Boolean(toml_edit::Formatted::new(false)));
    }

    // Try integer
    if let Ok(i) = trimmed.parse::<i64>() {
        return Ok(Value::Integer(toml_edit::Formatted::new(i)));
    }

    // Try float
    if let Ok(f) = trimmed.parse::<f64>() {
        return Ok(Value::Float(toml_edit::Formatted::new(f)));
    }

    // Default to string
    Ok(Value::String(toml_edit::Formatted::new(s.to_string())))
}

/// Show the config file path.
pub fn show_path() -> Result<()> {
    let config_path = config::config_file_path();
    println!("{}", config_path.display());
    Ok(())
}

/// Show example configuration.
pub fn show_example() -> Result<()> {
    print!("{}", tessitura_etl::config::example_config());
    Ok(())
}

/// Initialize config file with defaults.
pub fn init_config() -> Result<()> {
    let created = tessitura_etl::config::ensure_config_file()?;
    let config_path = config::config_file_path();

    if created {
        println!("✓ Created config file: {}", config_path.display());
        println!("\nEdit this file to configure tessitura.");
    } else {
        println!("Config file already exists: {}", config_path.display());
    }

    Ok(())
}
