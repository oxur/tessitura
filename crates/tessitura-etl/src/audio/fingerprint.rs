use anyhow::{Context, Result};
use rusty_chromaprint::{Configuration, Fingerprinter};
use std::io::Write;
use std::path::Path;

use super::decoder::decode_audio;

/// Encode a chromaprint fingerprint vector into a compressed base64 string.
///
/// AcoustID expects fingerprints in this format for lookup.
fn encode_fingerprint(fp: &[u32]) -> Result<String> {
    // Convert u32 array to bytes (little-endian)
    let mut bytes = Vec::with_capacity(fp.len() * 4);
    for &val in fp {
        bytes.extend_from_slice(&val.to_le_bytes());
    }

    // Base64 encode
    let mut compressed = Vec::new();
    let mut encoder =
        flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::default());
    encoder.write_all(&bytes)
        .context("Failed to write fingerprint data to encoder")?;
    encoder.finish()
        .context("Failed to finish encoding fingerprint")?;

    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, compressed))
}

/// Generate a chromaprint fingerprint for an audio file.
///
/// Returns the fingerprint string and duration in seconds.
pub fn generate_fingerprint(path: &Path) -> Result<(String, f64)> {
    // 1. Decode audio to mono PCM at 11025 Hz (chromaprint's preferred rate)
    let audio = decode_audio(path, 11025)
        .with_context(|| format!("Failed to decode audio: {}", path.display()))?;

    // 2. Create chromaprint fingerprinter
    let config = Configuration::preset_test2(); // Balanced preset for music
    let mut fpr = Fingerprinter::new(&config);

    // 3. Convert f32 samples to i16 (chromaprint expects i16)
    #[allow(clippy::cast_possible_truncation)]
    let samples_i16: Vec<i16> = audio
        .samples
        .iter()
        .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect();

    // 4. Feed samples to fingerprinter (mono = 1 channel)
    fpr.start(audio.sample_rate, 1)
        .context("Failed to start fingerprinter")?;

    fpr.consume(&samples_i16);

    fpr.finish();

    // 4. Get the fingerprint
    let fingerprint_vec = fpr.fingerprint();

    // Convert fingerprint vector to base64-encoded string
    let fingerprint = encode_fingerprint(&fingerprint_vec)?;

    Ok((fingerprint, audio.duration_secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_fingerprint_nonexistent_file() {
        let result = generate_fingerprint(Path::new("/nonexistent/file.mp3"));
        assert!(result.is_err());
        let err_string = result.unwrap_err().to_string();
        // Error message could be "Failed to decode audio" wrapping "Failed to open audio file"
        assert!(
            err_string.contains("Failed to decode audio")
                || err_string.contains("Failed to open audio file"),
            "Expected error about file, got: {}",
            err_string
        );
    }

    // Note: Real audio file tests would require test fixtures
    // For now, we test error handling
}
