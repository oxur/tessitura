use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio as mono PCM samples at a specific sample rate.
#[derive(Debug)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: f64,
}

/// Decode an audio file to mono PCM samples.
///
/// Resamples to target_sample_rate (typically 11025 or 16000 Hz for chromaprint).
/// Converts stereo to mono by averaging channels.
pub fn decode_audio(path: &Path, target_sample_rate: u32) -> Result<DecodedAudio> {
    // 1. Open the media source
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), symphonia::core::io::MediaSourceStreamOptions::default());

    // 2. Probe the format
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("Failed to probe audio format")?;

    let mut format = probed.format;

    // 3. Find the default audio track
    let track = format
        .default_track()
        .context("No default audio track found")?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    // 4. Create decoder
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    // 5. Decode all packets
    let mut sample_buf = None;
    let mut all_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e).context("Failed to read packet"),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                if sample_buf.is_none() {
                    let spec = *audio_buf.spec();
                    let duration = audio_buf.capacity() as u64;
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(ref mut buf) = sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    all_samples.extend_from_slice(buf.samples());
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {}
            Err(e) => return Err(e).context("Failed to decode packet"),
        }
    }

    // 6. Convert to mono if stereo (average channels)
    let channels = codec_params.channels.map_or(1, |c| c.count());
    #[allow(clippy::cast_precision_loss)]
    let mono_samples = if channels > 1 {
        all_samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        all_samples
    };

    // 7. Resample if needed (simplified - for production, use a proper resampler)
    let source_rate = codec_params.sample_rate.unwrap_or(44100);
    let resampled = if source_rate == target_sample_rate {
        mono_samples
    } else {
        resample_simple(&mono_samples, source_rate, target_sample_rate)
    };

    #[allow(clippy::cast_precision_loss)]
    let duration = resampled.len() as f64 / f64::from(target_sample_rate);

    Ok(DecodedAudio {
        samples: resampled,
        sample_rate: target_sample_rate,
        duration_secs: duration,
    })
}

/// Simple linear resampling (for production, consider using `rubato` or similar).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn resample_simple(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = f64::from(from_rate) / f64::from(to_rate);
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        if idx + 1 < samples.len() {
            let frac = pos - idx as f64;
            let sample = samples[idx].mul_add(1.0 - frac as f32, samples[idx + 1] * frac as f32);
            output.push(sample);
        } else if idx < samples.len() {
            output.push(samples[idx]);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_identity() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let resampled = resample_simple(&samples, 44100, 44100);
        assert_eq!(resampled, samples);
    }

    #[test]
    fn test_resample_downsample() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let resampled = resample_simple(&samples, 44100, 22050);
        // Should produce roughly 2 samples
        assert_eq!(resampled.len(), 2);
    }

    #[test]
    fn test_resample_upsample() {
        let samples = vec![1.0, 2.0];
        let resampled = resample_simple(&samples, 22050, 44100);
        // Should produce roughly 4 samples
        assert_eq!(resampled.len(), 4);
    }
}
