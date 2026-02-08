use anyhow::{Context, Result};
use tessitura_etl::{config, Config};

/// Show the current effective configuration.
pub fn show_config() -> Result<()> {
    let config = Config::load()?;

    println!("Current Configuration");
    println!("=====================\n");

    println!("Config file: {}", config::config_file_path().display());

    let exists = config::config_file_path().exists();
    println!("File exists: {}\n", if exists { "yes" } else { "no (using defaults)" });

    println!("Settings:");
    println!("  acoustid_api_key: {}",
        config.acoustid_api_key.as_deref().unwrap_or("<not set>"));
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
        let config = Config::load()?;

        match key.as_str() {
            "acoustid_api_key" => {
                println!("{}", config.acoustid_api_key.unwrap_or_else(|| String::from("<not set>")));
            }
            "database_path" => {
                println!("{}", config.database_path.display());
            }
            _ => {
                anyhow::bail!("Unknown config key: {}\n\nValid keys: acoustid_api_key, database_path", key);
            }
        }
    } else {
        // No key provided, show entire config file contents
        let config_path = config::config_file_path();

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            print!("{}", contents);
        } else {
            println!("Config file does not exist: {}", config_path.display());
            println!("\nRun 'tessitura config init' to create it.");
        }
    }

    Ok(())
}

/// Set a config value.
pub fn set_config(key: String, value: String) -> Result<()> {
    let config_path = config::config_file_path();

    // Ensure config file exists
    config::ensure_config_file()?;

    // Read existing config
    let mut contents = std::fs::read_to_string(&config_path)
        .context("Failed to read config file")?;

    // Update the value
    match key.as_str() {
        "acoustid_api_key" => {
            // Find and replace the acoustid_api_key line
            let lines: Vec<&str> = contents.lines().collect();
            let mut new_lines = Vec::new();
            let mut found = false;

            for line in lines {
                let trimmed = line.trim();
                if trimmed.starts_with("acoustid_api_key") && !trimmed.starts_with('#') {
                    new_lines.push(format!("acoustid_api_key = \"{}\"", value));
                    found = true;
                } else {
                    new_lines.push(line.to_string());
                }
            }

            if !found {
                // Add it at the end
                new_lines.push(format!("\nacoustid_api_key = \"{}\"", value));
            }

            contents = new_lines.join("\n");
        }
        "database_path" => {
            // Similar logic for database_path
            let lines: Vec<&str> = contents.lines().collect();
            let mut new_lines = Vec::new();
            let mut found = false;

            for line in lines {
                let trimmed = line.trim();
                if trimmed.starts_with("database_path") && !trimmed.starts_with('#') {
                    new_lines.push(format!("database_path = \"{}\"", value));
                    found = true;
                } else {
                    new_lines.push(line.to_string());
                }
            }

            if !found {
                // Add it at the end
                new_lines.push(format!("\ndatabase_path = \"{}\"", value));
            }

            contents = new_lines.join("\n");
        }
        _ => {
            anyhow::bail!("Unknown config key: {}\n\nValid keys: acoustid_api_key, database_path", key);
        }
    }

    // Write back
    std::fs::write(&config_path, contents)
        .context("Failed to write config file")?;

    println!("✓ Updated {} = {}", key, value);
    println!("  in {}", config_path.display());

    Ok(())
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
