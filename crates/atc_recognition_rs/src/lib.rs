//! ATC Recognition Library
//!
//! A library for recognizing aviation commands from voice input using Whisper
//! and parsing them into structured aviation command types.

use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

pub mod errors;
pub mod parser;
pub mod recognition;
pub mod speech_to_text;

pub use errors::Error;
pub use parser::{
    AviationCommandParser, CallsignMatch, CommandWithConfidence, ParseResult, ParsedCommand,
};
pub use recognition::VoiceRecognizer;
pub use speech_to_text::SpeechToText;

// Re-export specific aviation command types for convenience
pub use aviation_helper_rs::clearance::aviation_command::AviationCommandPart;

const SAMPLE_RATE_HZ: u32 = 16000;
const WHISPER_NUM_THREADS: i32 = 2;

pub fn create_resampler(sample_rate_in: u32) -> SincFixedIn<f32> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.99,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let resample_ratio = SAMPLE_RATE_HZ as f64 / sample_rate_in as f64;
    SincFixedIn::<f32>::new(resample_ratio, 2.0, params, 1024, 1).unwrap()
}

/// Configuration for voice recognition
#[derive(Debug, Clone)]
pub struct RecognitionConfig {
    pub model_path: &'static str,
    pub window_len_seconds: u32,
    pub check_interval_ms: u64,
    pub probability_threshold: f32,
    pub max_snippet_len_seconds: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            model_path: concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/models/whisper-small.en-atc-experiment/whisper-atc-q8_0.bin"
            ),
            window_len_seconds: 20,
            check_interval_ms: 3000,
            probability_threshold: 0.95,
            max_snippet_len_seconds: 17.0,
        }
    }
}
