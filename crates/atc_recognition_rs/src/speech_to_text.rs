//! Speech-to-text module for converting audio to text using Whisper
//!
//! This module handles the low-level speech recognition functionality,
//! providing a clean interface for converting audio samples to text.

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{Error, SpeechToTextConfig};

const SAMPLE_RATE_HZ: u32 = 16000;

/// Speech-to-text engine using Whisper for transcription
pub struct SpeechToText {
    pub whisper_context: WhisperContext,
}

impl SpeechToText {
    /// Create a new speech-to-text engine with the given configuration
    pub fn new(config: SpeechToTextConfig) -> Result<Self, Error> {
        let SpeechToTextConfig { model_path } = config;
        let whisper_context = WhisperContext::new_with_params(
            model_path.as_os_str().to_string_lossy().as_ref(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| Error::WhisperError(format!("Failed to create Whisper context: {}", e)))?;
        Ok(Self { whisper_context })
    }

    /// Get the expected sample rate for this speech-to-text engine
    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE_HZ
    }

    pub fn transcribe_with_whisper(&self, samples: &[f32]) -> Result<String, Error> {
        let mut state = self
            .whisper_context
            .create_state()
            .map_err(|e| Error::WhisperError(format!("Failed to create Whisper state: {}", e)))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(crate::WHISPER_NUM_THREADS);
        params.set_translate(false);
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run inference
        state
            .full(params, samples)
            .map_err(|e| Error::WhisperError(format!("Whisper inference failed: {}", e)))?;

        // Get transcribed text by concatenating all segments
        let mut result = String::new();
        for segment in state.as_iter() {
            let Ok(segment_text) = segment.to_str() else {
                continue;
            };
            result.push_str(segment_text);
        }
        let result = post_process_command(result);
        // Trim whitespace and return
        Ok(result)
    }
}

fn post_process_command(command: impl AsRef<str>) -> String {
    let ret: String = command.as_ref().trim().to_owned();
    ret.to_lowercase()
        .replace(".", "")
        .replace(",", "")
        .replace("!", "")
        .replace("?", "")
}
