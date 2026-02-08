# Tessitura Configuration

Tessitura supports multiple configuration methods with a clear priority order.

## Priority Order

Configuration sources are merged in the following order (highest to lowest priority):

1. **CLI arguments** - Flags passed on the command line
2. **Environment variables** - Variables with `TESS_` prefix
3. **Config file** - TOML file at `~/.config/tessitura/config.toml`
4. **Built-in defaults** - Hardcoded fallback values

## Configuration File

### Location

The configuration file location varies by platform:

- **Linux**: `~/.config/tessitura/config.toml`
- **macOS**: `~/Library/Application Support/tessitura/config.toml`
- **Windows**: `%APPDATA%\tessitura\config.toml`

Run `tessitura config path` to see the exact location on your system.

### Creating the Config File

The easiest way to create a config file is to use the CLI:

```bash
# Create default config file
tessitura config init

# View the config file location
tessitura config path

# View current configuration
tessitura config
```

### Managing Configuration

Use the `tessitura config` command to manage your configuration:

```bash
# Show current effective configuration
tessitura config

# Get a specific value
tessitura config get acoustid_api_key

# Set a value
tessitura config set acoustid_api_key "your-api-key"

# Show example configuration
tessitura config example

# View raw config file
tessitura config get
```

## Environment Variables

All configuration can be set via environment variables with the `TESS_` prefix:

```bash
# AcoustID API key
export TESS_ACOUSTID_API_KEY="your-api-key-here"

# Database path
export TESS_DATABASE_PATH="/custom/path/tessitura.db"
```

### Naming Convention

- Use `TESS_` prefix
- Convert field names to SCREAMING_SNAKE_CASE
- Example: `acoustid_api_key` → `TESS_ACOUSTID_API_KEY`

## CLI Arguments

Command-line arguments always take highest priority:

```bash
# Custom database path
tessitura --db /tmp/test.db scan /music

# Even if TESS_DATABASE_PATH is set, --db takes precedence
```

## Configuration Options

### `acoustid_api_key`

**Type**: String (optional)

AcoustID API key for fingerprint-based music identification.

**How to get**:
1. Visit https://acoustid.org/new-application
2. Register a free account
3. Create an application
4. Copy your API key

**Priority**:
- CLI: Not available (use env var or config file)
- ENV: `TESS_ACOUSTID_API_KEY`
- Config: `acoustid_api_key = "..."`
- Default: None

**Example**:
```toml
acoustid_api_key = "a1b2c3d4e5f6"
```

### `database_path`

**Type**: Path

Path to the SQLite database file where tessitura stores all catalog data.

**Priority**:
- CLI: `--db /path/to/db`
- ENV: `TESS_DATABASE_PATH`
- Config: `database_path = "/path/to/db"`
- Default: `~/.local/share/tessitura/tessitura.db`

**Example**:
```toml
database_path = "/media/music-archive/tessitura.db"
```

## Verification

To verify your configuration is loaded correctly:

```bash
# Check that the database path is used
tessitura scan --help
# Shows: Path to the database (default: ~/.local/share/tessitura/tessitura.db)

# Run with environment variable
TESS_ACOUSTID_API_KEY=test ./bin/tessitura identify
# Should show: Starting identification
```

## Troubleshooting

### Config file not found

The config file is optional. If it doesn't exist, tessitura will use environment variables and defaults.

### Invalid TOML syntax

If you see parsing errors, check your config file:

```bash
# Validate TOML syntax
cat ~/.config/tessitura/config.toml
```

Common issues:
- Missing quotes around strings
- Typos in field names
- Invalid paths with unescaped backslashes (Windows)

### Environment variables not working

Ensure you're using the correct `TESS_` prefix:

```bash
# ❌ Wrong
export ACOUSTID_API_KEY="..."

# ✅ Correct
export TESS_ACOUSTID_API_KEY="..."
```

### Priority confusion

Remember: CLI > ENV > Config > Defaults

```bash
# Config file has: database_path = "/config/path/db"
# ENV has: TESS_DATABASE_PATH="/env/path/db"
# CLI has: --db /cli/path/db

# Result: Uses /cli/path/db (CLI wins)
```

## Migration from v0.1.0

The old `ACOUSTID_API_KEY` environment variable (without `TESS_` prefix) is no longer supported. Update to:

```bash
# Old (v0.1.0)
export ACOUSTID_API_KEY="..."

# New (v0.2.0+)
export TESS_ACOUSTID_API_KEY="..."
```

Or use the config file instead.
