//! Test utilities for WAV file processing
//! Available only when testing feature is enabled
use std::path::Path;

use super::*;

/// Process a WAV file and return the recognized text and parsed commands
/// This function is only available for tests
pub fn process_wav_file_for_test(
    recognizer: &SpeechToText,
    wav_path: impl AsRef<Path>,
) -> Result<String, Error> {
    let samples = read_wav_file(wav_path.as_ref())?;
    recognizer.transcribe_with_whisper(&samples)
}
/// Read and process a WAV file, returning the audio samples
/// This function is only available for tests
pub fn read_wav_file(wav_path: impl AsRef<Path>) -> Result<Vec<f32>, Error> {
    let path = wav_path.as_ref();
    // Read WAV file using hound
    let mut reader = hound::WavReader::open(path).map_err(|e| {
        Error::StdIo(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    let spec = reader.spec();
    println!(
        "WAV file specs: sample_rate={}, channels={}, bits_per_sample={}",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Read and process audio samples
    read_audio_samples_from_reader(&mut reader)
}

fn read_audio_samples_from_reader(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
) -> Result<Vec<f32>, Error> {
    let spec = reader.spec();

    // Convert to f32 samples
    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let int_samples: Result<Vec<i32>, _> = reader.samples::<i32>().collect();
            let int_samples = int_samples.map_err(|e| {
                Error::StdIo(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))
            })?;

            // Convert i32 to f32 samples (normalized to -1.0 to 1.0)
            let max_int = (1i64 << (spec.bits_per_sample - 1)) as f32;
            Ok(int_samples
                .iter()
                .map(|&sample| sample as f32 / max_int)
                .collect())
        }
    };

    let mut samples = samples.map_err(|e| {
        Error::StdIo(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    // Resample to 16kHz if needed (Whisper expects 16kHz)
    if spec.sample_rate != SAMPLE_RATE_HZ {
        println!(
            "Resampling from {}Hz to {}Hz",
            spec.sample_rate, SAMPLE_RATE_HZ
        );
        samples = resample_audio_simple(samples, spec.sample_rate, SAMPLE_RATE_HZ)?;
    }

    // Convert stereo to mono if needed
    if spec.channels == 2 {
        println!("Converting stereo to mono");
        samples = samples
            .chunks_exact(2)
            .map(|chunk| (chunk[0] + chunk[1]) / 2.0)
            .collect();
    }

    Ok(samples)
}

fn resample_audio_simple(input: Vec<f32>, from_rate: u32, to_rate: u32) -> Result<Vec<f32>, Error> {
    if from_rate == to_rate {
        return Ok(input);
    }

    // Simple resampling for tests
    let ratio = to_rate as f64 / from_rate as f64;
    let output_len = (input.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_index = (i as f64 / ratio) as usize;
        if src_index < input.len() {
            output.push(input[src_index]);
        } else {
            output.push(0.0);
        }
    }

    Ok(output)
}
