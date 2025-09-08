//! ATC Recognition Library
//!
//! A library for recognizing aviation commands from voice input using Whisper
//! and parsing them into structured aviation command types.

use std::path::Path;

use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

pub mod errors;
pub mod parser;
pub mod graph_parser;
// pub mod recognition;
pub mod speech_to_text;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use errors::Error;
pub use parser::{
    AviationCommandParser, CallsignMatch, CommandWithConfidence, ParseResult, ParsedCommand,
};
pub use graph_parser::{
    GraphParser, GraphParseResult, GraphParsedCommand, GraphCommandWithConfidence,
};
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
pub struct SpeechToTextConfig {
    pub model_path: &'static Path,
}

impl Default for SpeechToTextConfig {
    fn default() -> Self {
        Self {
            model_path: Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/resources/models/whisper.cpp/ggml-medium.en-q5_0.bin"
            )),
        }
    }
}
