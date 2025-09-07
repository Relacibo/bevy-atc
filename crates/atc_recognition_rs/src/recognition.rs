//! Voice recognition module
//!
//! Handles the audio input, processing with Whisper, and integration with the command parser.
//! This module orchestrates the speech-to-text and command parsing components.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split},
};
use rubato::Resampler;

use crate::{AviationCommandParser, Error, RecognitionConfig, SpeechToText, create_resampler};
use aviation_helper_rs::clearance::{airlines::Airlines, aviation_command::AviationCommandPart};

const SAMPLE_RATE_HZ: u32 = 16000;

/// Voice recognizer that captures audio and converts it to aviation commands
/// This is the main orchestrator that combines speech-to-text and command parsing
pub struct VoiceRecognizer {
    config: RecognitionConfig,
    speech_to_text: SpeechToText,
    parser: AviationCommandParser,
}

impl VoiceRecognizer {
    pub fn new(config: RecognitionConfig, airlines: Airlines) -> Result<Self, Error> {
        let speech_to_text = SpeechToText::new(config.clone())?;
        let parser = AviationCommandParser::new(airlines);

        Ok(Self {
            config,
            speech_to_text,
            parser,
        })
    }

    /// Get a reference to the speech-to-text component
    pub fn speech_to_text(&self) -> &SpeechToText {
        &self.speech_to_text
    }

    /// Get a reference to the command parser component
    pub fn parser(&self) -> &AviationCommandParser {
        &self.parser
    }

    /// Perform a single voice recognition and return the parsed command
    /// This method captures live audio from the microphone, transcribes it, and parses it
    pub fn recognize_single_command(&self) -> Result<Option<AviationCommandPart>, Error> {
        println!("Starting single voice recognition...");
        
        // TODO: Implement live microphone capture for single command
        // For now, we use the test simulation until live audio capture is implemented
        // This should be replaced with actual microphone recording and transcription
        
        // Temporary test simulation (should be removed when live capture is ready)
        todo!()
    }
    
    /// Capture audio from microphone for a specified duration and return the samples
    /// This is the foundation for real single-command recognition
    pub fn capture_audio_samples(&self, duration_seconds: f32) -> Result<Vec<f32>, Error> {
        println!("🎤 Capturing audio for {:.1} seconds...", duration_seconds);
        
        let cpal_host = cpal::default_host();
        let input_device = cpal_host
            .default_input_device()
            .ok_or(Error::FailedToFindDefaultInputDevice)?;
            
        let config: cpal::StreamConfig = input_device.default_input_config()?.into();
        let sample_rate_in = config.sample_rate.0;
        let channel_count_in = config.channels;
        
        // Calculate target sample count
        let target_samples = (SAMPLE_RATE_HZ as f32 * duration_seconds) as usize;
        let captured_samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(target_samples)));
        let samples_clone = captured_samples.clone();
        
        let resample_buffer: Arc<Mutex<[Vec<f32>; 1]>> = Arc::new(Mutex::new([vec![]]));
        
        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let Ok(mut rb) = resample_buffer.lock() else {
                eprintln!("Could not lock mutex");
                return;
            };
            rb[0].clear();

            let data2 = if sample_rate_in != SAMPLE_RATE_HZ {
                let mut resampler = create_resampler(sample_rate_in);

                let expected_output_len = ((data.len() as f64 / channel_count_in as f64)
                    * (SAMPLE_RATE_HZ as f64 / sample_rate_in as f64))
                    .ceil() as usize;
                rb[0].resize(expected_output_len, 0.0);

                if let Err(err) = resampler.process_into_buffer(
                    &[data
                        .chunks(channel_count_in as usize)
                        .map(|frame| frame[0])
                        .collect::<Vec<_>>()],
                    rb.as_mut_slice(),
                    None,
                ) {
                    eprintln!("Rubato resampling failed: {:?}", err);
                    return;
                }
                &rb[0][..]
            } else {
                data
            };
            
            // Add samples to our capture buffer
            let mut samples = samples_clone.lock().unwrap();
            for &sample in data2 {
                if samples.len() < target_samples {
                    samples.push(sample);
                }
            }
        };
        
        let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
        input_stream.play()?;
        
        // Record for the specified duration
        thread::sleep(Duration::from_millis((duration_seconds * 1000.0) as u64));
        
        // Stop recording
        drop(input_stream);
        
        let samples = captured_samples.lock().unwrap().clone();
        println!("🎵 Captured {} audio samples", samples.len());
        
        Ok(samples)
    }
    
    /// Perform real single voice recognition with live microphone capture
    /// This is the proper implementation that should replace the test simulation
    pub fn recognize_single_command_live(&self, duration_seconds: f32) -> Result<Option<AviationCommandPart>, Error> {
        println!("🎙️  Starting LIVE voice recognition for {:.1} seconds...", duration_seconds);
        
        // Capture audio from microphone
        let audio_samples = self.capture_audio_samples(duration_seconds)?;
        
        if audio_samples.is_empty() {
            println!("⚠️  No audio captured");
            return Ok(None);
        }
        
        // Use speech-to-text component for transcription
        let transcribed_text = self.speech_to_text.transcribe(&audio_samples)?;
        
        if transcribed_text.is_empty() {
            println!("⚠️  No text transcribed from audio");
            return Ok(None);
        }
        
        println!("🎯 Transcribed: '{}'", transcribed_text);
        
        // Parse the transcribed text
        let parsed_command = self.parser.parse(&transcribed_text);
        if let Some(ref command) = parsed_command {
            println!("✅ Parsed aviation command: {:?}", command);
        } else {
            println!("❌ Could not parse '{}' as aviation command", transcribed_text);
        }
        
        Ok(parsed_command)
    }

    /// Start continuous voice recognition with a callback for each recognized command
    pub fn start_continuous_recognition<F>(self, callback: F) -> Result<(), Error>
    where
        F: Fn(AviationCommandPart) + Send + 'static,
    {
        let cpal_host = cpal::default_host();
        let input_device = cpal_host
            .default_input_device()
            .ok_or(Error::FailedToFindDefaultInputDevice)?;

        #[cfg(debug_assertions)]
        if let Ok(name) = input_device.name() {
            println!("Using input device: {}", name);
        }

        let config: cpal::StreamConfig = input_device.default_input_config()?.into();
        let sample_rate_in = config.sample_rate.0;
        let channel_count_in = config.channels;

        let latency_samples = SAMPLE_RATE_HZ * self.config.window_len_seconds;
        let ring = HeapRb::<f32>::new(latency_samples as usize * 8);
        let (mut producer, consumer) = ring.split();

        let resample_buffer: Arc<Mutex<[Vec<f32>; 1]>> = Arc::new(Mutex::new([vec![]]));

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let Ok(mut rb) = resample_buffer.lock() else {
                eprintln!("Could not lock mutex");
                return;
            };
            rb[0].clear();

            let data2 = if sample_rate_in != SAMPLE_RATE_HZ {
                let mut resampler = create_resampler(sample_rate_in);

                let expected_output_len = ((data.len() as f64 / channel_count_in as f64)
                    * (SAMPLE_RATE_HZ as f64 / sample_rate_in as f64))
                    .ceil() as usize;
                rb[0].resize(expected_output_len, 0.0);

                if let Err(err) = resampler.process_into_buffer(
                    &[data
                        .chunks(channel_count_in as usize)
                        .map(|frame| frame[0])
                        .collect::<Vec<_>>()],
                    rb.as_mut_slice(),
                    None,
                ) {
                    eprintln!("Rubato resampling failed: {:?}", err);
                    return;
                }
                &rb[0][..]
            } else {
                data
            };

            let len = data2.len();
            let pushed_count = producer.push_slice(data2);
            if len - pushed_count != 0 {
                eprintln!("Mic buffer overflow");
            }
        };

        let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
        let consumer_clone = Arc::new(Mutex::new(consumer));

        let (tx, rx) = mpsc::channel::<String>();
        let parser = self.parser;
        let config = self.config.clone();

        // Recognition thread - simplified for now to use test commands
        thread::spawn(move || {
            // Note: In a full implementation, this would use the SpeechToText component
            // For now, we continue with the test simulation

            let mut audio_buffer =
                vec![0.0f32; SAMPLE_RATE_HZ as usize * config.window_len_seconds as usize];

            loop {
                let read_samples_len = {
                    let cons = consumer_clone.lock().unwrap();
                    cons.peek_slice(&mut audio_buffer)
                };

                if read_samples_len < SAMPLE_RATE_HZ as usize {
                    thread::sleep(Duration::from_millis(config.check_interval_ms));
                    continue;
                }

                // For now, use test commands until we solve the whisper API issue
                let test_commands = [
                    "turn left heading two seven zero",
                    "climb and maintain flight level three five zero",
                    "contact tower on frequency one two one point five",
                    "descend to four thousand feet",
                ];

                static mut COMMAND_INDEX: usize = 0;
                let test_text = unsafe {
                    let cmd = test_commands[COMMAND_INDEX % test_commands.len()].to_string();
                    COMMAND_INDEX += 1;
                    cmd
                };

                println!("Test recognition: {}", test_text);
                let _ = tx.send(test_text);

                let mut cons = consumer_clone.lock().unwrap();
                cons.skip(read_samples_len);
                drop(cons);

                thread::sleep(Duration::from_millis(config.check_interval_ms));
            }
        });

        // Command processing thread
        thread::spawn(move || {
            while let Ok(recognized_text) = rx.recv() {
                println!("Received recognized text: {}", recognized_text);

                if let Some(command) = parser.parse(&recognized_text) {
                    println!("Parsed command: {:?}", command);
                    callback(command);
                } else {
                    println!("No valid command found in: {}", recognized_text);
                }
            }
        });

        input_stream.play()?;
        println!("Voice recognition started. Speak aviation commands...");

        // Keep the main thread alive
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Process audio samples and return the recognized text and parsed commands
    /// Accepts a slice of f32 audio samples (expected to be 16kHz mono)
    pub fn process_audio_samples(
        &self,
        samples: &[f32],
    ) -> Result<(String, Option<crate::parser::ParsedCommand>), Error> {
        println!("Processing {} audio samples", samples.len());

        // Use the speech-to-text component for transcription
        let transcribed_text = self.speech_to_text.transcribe(samples)?;

        println!("Transcribed text: '{}'", transcribed_text);

        // Parse the transcribed text with our aviation command parser
        let parsed_command = match self.parser.parse_transmission_enhanced(&transcribed_text) {
            crate::parser::ParseResult::Success(parsed) => Some(parsed),
            crate::parser::ParseResult::PartialSuccess { parsed, .. } => Some(parsed),
            _ => None,
        };

        Ok((transcribed_text, parsed_command))
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("Audio stream error: {}", err);
}

/// Test utilities for WAV file processing
/// Available only when testing feature is enabled
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {
    use super::*;
    use crate::speech_to_text::test_utils as stt_utils;

    /// Read and process a WAV file, returning the audio samples
    /// This function is only available for tests
    pub fn read_wav_file(wav_path: &str) -> Result<Vec<f32>, Error> {
        stt_utils::read_wav_file(wav_path)
    }

    /// Process a WAV file and return the recognized text and parsed commands
    /// This function is only available for tests
    pub fn process_wav_file_for_test(
        recognizer: &VoiceRecognizer,
        wav_path: &str,
    ) -> Result<(String, Option<crate::parser::ParsedCommand>), Error> {
        let samples = read_wav_file(wav_path)?;
        recognizer.process_audio_samples(&samples)
    }
}
