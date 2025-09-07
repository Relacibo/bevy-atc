//! Speech-to-text module for converting audio to text using Whisper
//!
//! This module handles the low-level speech recognition functionality,
//! providing a clean interface for converting audio samples to text.

use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{Error, SpeechToTextConfig};

const SAMPLE_RATE_HZ: u32 = 16000;

/// Speech-to-text engine using Whisper for transcription
pub struct SpeechToText {
    pub window_len_seconds: u32,
    pub check_interval_ms: u64,
    pub probability_threshold: f32,
    pub max_snippet_len_seconds: f32,
    pub whisper_context: WhisperContext,
}

impl SpeechToText {
    /// Create a new speech-to-text engine with the given configuration
    pub fn new(config: SpeechToTextConfig) -> Result<Self, Error> {
        let SpeechToTextConfig {
            model_path,
            window_len_seconds,
            check_interval_ms,
            probability_threshold,
            max_snippet_len_seconds,
        } = config;
        let whisper_context = WhisperContext::new_with_params(
            model_path.as_os_str().to_string_lossy().as_ref(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| Error::WhisperError(format!("Failed to create Whisper context: {}", e)))?;
        Ok(Self {
            window_len_seconds,
            check_interval_ms,
            probability_threshold,
            max_snippet_len_seconds,
            whisper_context,
        })
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
        let num_segments = state.full_n_segments();
        let mut result = String::new();
        todo!()
        // for i in 0..num_segments {
        //     let segment_text = state
        //         .full_get_segment_text(i)
        //         .map_err(|e| Error::WhisperError(format!("Failed to get segment text: {}", e)))?;

        //     if !result.is_empty() && !segment_text.is_empty() {
        //         result.push(' ');
        //     }
        //     result.push_str(&segment_text);
        // }

        // // Trim whitespace and return
        // Ok(result.trim().to_string())
    }
}
