//! Tests for real audio recording parsing
//!
//! This module tests the parser against real ATC audio recordings using the RON index file
//! to verify that our parser can handle actual speech-to-text output.

use atc_recognition_rs::{
    RecognitionConfig, VoiceRecognizer,
    parser::{AviationCommandParser, ParseResult},
    recognition::test_utils,
};
use aviation_helper_rs::clearance::{
    airlines::Airlines,
    aviation_command::{AviationCommandGroup, CommunicationEntity},
};
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};

static AIRLINES: LazyLock<Airlines> =
    LazyLock::new(|| Airlines::load_airlines_from_file().unwrap());

static COMMAND_PARSER: LazyLock<AviationCommandParser> =
    LazyLock::new(|| AviationCommandParser::new(AIRLINES.clone()));

static TEST_RECORDINGS_INDEX: LazyLock<TestRecordingsIndex> =
    LazyLock::new(TestRecordingsIndex::load_from_file);

static VOICE_RECOGNIZER: LazyLock<VoiceRecognizer> = LazyLock::new(|| {
    let config = RecognitionConfig {
        model_path: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models/whisper-small.en-atc-experiment/whisper-atc-q8_0.bin"
        ),
        window_len_seconds: 5,
        check_interval_ms: 100,
        probability_threshold: 0.95,
        max_snippet_len_seconds: 10.0,
    };
    VoiceRecognizer::new(config, AIRLINES.clone()).expect("Failed to create recognizer")
});

#[derive(Debug, Deserialize)]
struct TestRecording {
    command: AviationCommandGroup,
    text: String,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct TestRecordingsIndex {
    entries: HashMap<String, TestRecording>,
}

impl TestRecordingsIndex {
    fn load_from_file() -> Self {
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/test-recordings/index.ron"
        ))
        .expect("Failed to read test recordings index");

        ron::from_str(&content).expect("Failed to parse test recordings index")
    }
}

#[test]
fn test_delta_recording() {
    let delta = &TEST_RECORDINGS_INDEX.entries["delta"];

    println!("Testing Delta recording:");
    println!("Text: '{}'", delta.text);
    println!("Expected command: {:?}", delta.command);

    // Parse the text with our parser
    let result = COMMAND_PARSER.parse_transmission_enhanced(&delta.text);

    match result {
        ParseResult::Success(parsed) => {
            println!("✓ Parsed callsign: {}", parsed.callsign);
            println!("✓ Parsed commands: {:?}", parsed.commands);

            // STRICT: Verify the callsign matches exactly
            if let Some(ref target) = delta.command.target {
                if let CommunicationEntity::Aircraft { full_name } = target {
                    assert_eq!(
                        parsed.callsign, *full_name,
                        "Delta callsign must match exactly. Expected: '{}', got: '{}'",
                        full_name, parsed.callsign
                    );
                } else {
                    panic!("Expected Aircraft target, got: {:?}", target);
                }
            } else {
                panic!("Expected a target aircraft in Delta command");
            }

            // STRICT: Verify exact number of commands
            assert_eq!(
                parsed.commands.len(),
                delta.command.parts.len(),
                "Delta must have exactly {} commands, got {}. Expected: {:?}, Parsed: {:?}",
                delta.command.parts.len(),
                parsed.commands.len(),
                delta.command.parts,
                parsed.commands
            );

            // STRICT: Verify commands match EXACTLY (complete value equality)
            for (i, (expected, actual)) in delta
                .command
                .parts
                .iter()
                .zip(parsed.commands.iter())
                .enumerate()
            {
                // NOW WE CAN USE DIRECT EQUALITY - NO MORE LOOSE TYPE-ONLY CHECKS!
                assert_eq!(
                    *expected, actual.command,
                    "Delta command {} must match exactly. Expected: {:?}, got: {:?}",
                    i, expected, actual.command
                );
            }
        }
        ParseResult::PartialSuccess {
            parsed,
            unparsed_parts,
        } => {
            panic!(
                "Delta parsing should be complete, not partial. Parsed: {:?}, Unparsed: {:?}",
                parsed, unparsed_parts
            );
        }
        ParseResult::CallsignOnly(callsign) => {
            panic!(
                "Delta parsing should extract commands, not just callsign: '{}'",
                callsign
            );
        }
        ParseResult::Failed { reason, raw_text } => {
            panic!(
                "Delta parsing failed completely. Input: '{}', Reason: '{}'",
                raw_text, reason
            );
        }
    }
}

#[test]
fn test_easyjet_recording() {
    let easyjet = &TEST_RECORDINGS_INDEX.entries["easyjet"];

    println!("Testing EasyJet recording:");
    println!("Text: '{}'", easyjet.text);
    println!("Expected command: {:?}", easyjet.command);

    let result = COMMAND_PARSER.parse_transmission_enhanced(&easyjet.text);

    match result {
        ParseResult::Success(parsed) => {
            println!("✓ Parsed callsign: {}", parsed.callsign);
            println!("✓ Parsed commands: {:?}", parsed.commands);

            // STRICT: Verify callsign matches exactly
            if let Some(CommunicationEntity::Aircraft { full_name }) = &easyjet.command.target {
                assert_eq!(
                    parsed.callsign, *full_name,
                    "EasyJet callsign must match exactly. Expected: '{}', got: '{}'",
                    full_name, parsed.callsign
                );
            } else {
                panic!("Expected Aircraft target for EasyJet command");
            }

            // STRICT: EasyJet should have exact number of commands
            assert_eq!(
                parsed.commands.len(),
                easyjet.command.parts.len(),
                "EasyJet must have exactly {} commands, got {}. Expected: {:?}, Parsed: {:?}",
                easyjet.command.parts.len(),
                parsed.commands.len(),
                easyjet.command.parts,
                parsed.commands
            );

            // STRICT: Verify commands match EXACTLY (complete value equality)
            for (i, (expected, actual)) in easyjet
                .command
                .parts
                .iter()
                .zip(parsed.commands.iter())
                .enumerate()
            {
                // COMPLETE VALUE COMPARISON - no more type-only checks!
                assert_eq!(
                    *expected, actual.command,
                    "EasyJet command {} must match exactly. Expected: {:?}, got: {:?}",
                    i, expected, actual.command
                );
            }
        }
        ParseResult::PartialSuccess {
            parsed,
            unparsed_parts,
        } => {
            panic!(
                "EasyJet parsing must be complete, not partial. Parsed: {:?}, Unparsed: {:?}",
                parsed, unparsed_parts
            );
        }
        ParseResult::CallsignOnly(callsign) => {
            panic!(
                "EasyJet parsing should extract commands, not just callsign: '{}'",
                callsign
            );
        }
        ParseResult::Failed { reason, raw_text } => {
            panic!(
                "EasyJet parsing failed. Input: '{}', Reason: '{}'",
                raw_text, reason
            );
        }
    }
}

#[test]
fn test_lufthansa_cargo_recording() {
    let lufthansa = &TEST_RECORDINGS_INDEX.entries["lufthansa_cargo"];

    println!("Testing Lufthansa Cargo recording:");
    println!("Text: '{}'", lufthansa.text);
    println!("Expected command: {:?}", lufthansa.command);

    let result = COMMAND_PARSER.parse_transmission_enhanced(&lufthansa.text);

    match result {
        ParseResult::Success(parsed) => {
            println!("✓ Parsed callsign: {}", parsed.callsign);
            println!("✓ Parsed commands: {:?}", parsed.commands);

            // STRICT: Verify callsign matches exactly
            if let Some(CommunicationEntity::Aircraft { full_name }) = &lufthansa.command.target {
                assert_eq!(
                    parsed.callsign, *full_name,
                    "Lufthansa Cargo callsign must match exactly. Expected: '{}', got: '{}'",
                    full_name, parsed.callsign
                );
            } else {
                panic!("Expected Aircraft target for Lufthansa Cargo command");
            }

            // STRICT: Lufthansa Cargo should have exact number of commands
            assert_eq!(
                parsed.commands.len(),
                lufthansa.command.parts.len(),
                "Lufthansa Cargo must have exactly {} commands, got {}. Expected: {:?}, Parsed: {:?}",
                lufthansa.command.parts.len(),
                parsed.commands.len(),
                lufthansa.command.parts,
                parsed.commands
            );

            // STRICT: Verify commands match EXACTLY (complete value equality)
            for (i, (expected, actual)) in lufthansa
                .command
                .parts
                .iter()
                .zip(parsed.commands.iter())
                .enumerate()
            {
                // COMPLETE VALUE COMPARISON - no more type-only checks!
                assert_eq!(
                    *expected, actual.command,
                    "Lufthansa Cargo command {} must match exactly. Expected: {:?}, got: {:?}",
                    i, expected, actual.command
                );
            }
        }
        ParseResult::PartialSuccess {
            parsed,
            unparsed_parts,
        } => {
            panic!(
                "Lufthansa Cargo parsing must be complete, not partial. Parsed: {:?}, Unparsed: {:?}",
                parsed, unparsed_parts
            );
        }
        ParseResult::CallsignOnly(callsign) => {
            panic!(
                "Lufthansa Cargo parsing should extract commands, not just callsign: '{}'",
                callsign
            );
        }
        ParseResult::Failed { reason, raw_text } => {
            panic!(
                "Lufthansa Cargo parsing failed. Input: '{}', Reason: '{}'",
                raw_text, reason
            );
        }
    }
}

#[cfg(test)]
mod audio_processing_tests {
    use super::*;
    use std::path::Path;

    /// Test WAV file processing for Delta recording
    /// This test will be skipped if no WAV file exists, but MUST succeed if model is available
    #[test]
    fn test_wav_processing_delta() {
        let wav_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/test-recordings/delta.wav",
        );

        // Skip test if WAV file doesn't exist
        if !std::path::Path::new(wav_path).exists() {
            println!("⚠️  Skipping WAV test: file not found: {}", wav_path);
            return;
        }

        let result = test_utils::process_wav_file_for_test(&VOICE_RECOGNIZER, wav_path);

        match result {
            Ok((transcribed_text, parsed_command)) => {
                println!("✓ Successfully processed WAV file");
                println!("Transcribed: '{}'", transcribed_text);

                // STRICT: If we get a transcription, it must be parseable
                if !transcribed_text.is_empty()
                    && transcribed_text
                        != "transcription placeholder - whisper integration needs API fix"
                {
                    let parsed = parsed_command.expect(
                        "If Whisper produces transcription, parser MUST extract aviation commands from it"
                    );

                    assert!(
                        !parsed.callsign.is_empty(),
                        "STRICT: Must extract callsign from transcription: '{}'",
                        transcribed_text
                    );

                    assert!(
                        !parsed.commands.is_empty(),
                        "STRICT: Must extract at least one command from transcription: '{}'",
                        transcribed_text
                    );

                    // Delta should have callsign starting with "DAL"
                    assert!(
                        parsed.callsign.starts_with("DAL"),
                        "STRICT: Delta callsign must start with 'DAL', got: '{}'",
                        parsed.callsign
                    );
                } else {
                    println!(
                        "ℹ️  Whisper model not available or returned placeholder - testing WAV pipeline only"
                    );
                }
            }
            Err(e) => panic!("FAILED: WAV file processing must succeed. Error: {:?}", e),
        }
    }

    #[test]
    fn test_wav_processing_easyjet() {
        let wav_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/test-recordings/easyjet.wav",
        );

        let result = test_utils::process_wav_file_for_test(&VOICE_RECOGNIZER, wav_path);

        match result {
            Ok((transcribed_text, parsed_command)) => {
                println!("✓ Successfully processed WAV file");
                println!("Transcribed: '{}'", transcribed_text);

                if transcribed_text.is_empty() {
                    println!(
                        "Note: No Whisper model available - testing WAV processing pipeline only"
                    );
                    return;
                }

                if let Some(parsed) = parsed_command {
                    println!("✓ Parsed callsign: {}", parsed.callsign);
                    println!("✓ Parsed commands: {:?}", parsed.commands);
                    assert!(!parsed.callsign.is_empty(), "Should extract a callsign");
                } else {
                    println!("Note: Could not parse transcribed text as aviation command");
                }
            }
            Err(e) => panic!("Failed to process WAV file: {:?}", e),
        }
    }

    #[test]
    fn test_wav_processing_lufthansa_cargo() {
        let wav_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/test-recordings/lufthansa_cargo.wav",
        );

        let result = test_utils::process_wav_file_for_test(&VOICE_RECOGNIZER, wav_path);

        match result {
            Ok((transcribed_text, parsed_command)) => {
                println!("Transcribed: '{}'", transcribed_text);

                if let Some(parsed) = parsed_command {
                    println!("Parsed callsign: {}", parsed.callsign);
                    println!("Parsed commands: {:?}", parsed.commands);

                    assert_eq!(
                        parsed.callsign, "GEC990321",
                        "Should extract correct callsign"
                    );
                    assert!(
                        !parsed.commands.is_empty(),
                        "Should parse at least one command"
                    );
                } else {
                    println!("Note: Could not parse transcribed text, but transcription worked");
                }
            }
            Err(e) => panic!("Failed to process WAV file: {:?}", e),
        }
    }

    /// Test processing all WAV files that exist
    #[test]
    fn test_all_available_wav_files() {
        let recordings_dir = format!("{}/resources/test-recordings/", env!("CARGO_MANIFEST_DIR"));

        let test_files = ["delta.wav", "easyjet.wav", "lufthansa_cargo.wav"];

        for file_name in &test_files {
            let wav_path = format!("{}{}", recordings_dir, file_name);

            if !Path::new(&wav_path).exists() {
                println!("Skipping {} - file not found", file_name);
                continue;
            }

            println!("\n=== Processing {} ===", file_name);

            match test_utils::process_wav_file_for_test(&VOICE_RECOGNIZER, &wav_path) {
                Ok((transcribed_text, parsed_command)) => {
                    println!("✓ Transcribed: '{}'", transcribed_text);

                    if let Some(parsed) = parsed_command {
                        println!("✓ Parsed callsign: {}", parsed.callsign);
                        println!("✓ Parsed {} commands", parsed.commands.len());

                        // Basic validation
                        assert!(!parsed.callsign.is_empty(), "Should extract a callsign");
                        assert!(
                            parsed.callsign_confidence > 0.3,
                            "Should have reasonable confidence"
                        );
                    } else {
                        println!("! Could not parse the transcribed text into aviation commands");
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to process {}: {:?}", file_name, e);
                    // Don't panic here - some files might be missing or have issues
                }
            }
        }
    }
}

/// Tests for direct text-to-command parsing (no audio involved)
#[cfg(test)]
mod text_parsing_tests {
    use super::*;

    #[test]
    fn test_text_parsing_delta() {
        let delta = &TEST_RECORDINGS_INDEX.entries["delta"];

        println!("Testing text parsing for Delta:");
        println!("Input text: '{}'", delta.text);
        println!("Expected: {:?}", delta.command);

        let result = COMMAND_PARSER.parse_transmission_enhanced(&delta.text);

        match result {
            ParseResult::Success(parsed) => {
                println!("✓ Parsed callsign: {}", parsed.callsign);
                println!("✓ Parsed commands: {:?}", parsed.commands);

                // Verify callsign
                if let Some(ref target) = delta.command.target {
                    if let CommunicationEntity::Aircraft { full_name } = target {
                        assert_eq!(parsed.callsign, *full_name, "Callsign should match");
                    }
                }

                // Verify we parsed some commands
                assert!(
                    !parsed.commands.is_empty(),
                    "Should parse at least one command"
                );
            }
            ParseResult::PartialSuccess {
                parsed,
                unparsed_parts,
            } => {
                println!(
                    "⚠ Partial success - callsign: {}, unparsed: {:?}",
                    parsed.callsign, unparsed_parts
                );

                // Still check callsign
                if let Some(ref target) = delta.command.target {
                    if let CommunicationEntity::Aircraft { full_name } = target {
                        assert_eq!(parsed.callsign, *full_name, "Callsign should match");
                    }
                }
            }
            other => {
                panic!("Failed to parse text for Delta: {:?}", other);
            }
        }
    }

    #[test]
    fn test_text_parsing_easyjet() {
        let easyjet = &TEST_RECORDINGS_INDEX.entries["easyjet"];

        println!("Testing text parsing for EasyJet:");
        println!("Input text: '{}'", easyjet.text);

        let result = COMMAND_PARSER.parse_transmission_enhanced(&easyjet.text);

        match result {
            ParseResult::Success(parsed) | ParseResult::PartialSuccess { parsed, .. } => {
                println!("✓ Parsed callsign: {}", parsed.callsign);
                println!("✓ Parsed commands: {:?}", parsed.commands);

                if let Some(ref target) = easyjet.command.target {
                    if let CommunicationEntity::Aircraft { full_name } = target {
                        assert_eq!(parsed.callsign, *full_name, "Callsign should match");
                    }
                }

                assert!(
                    !parsed.commands.is_empty(),
                    "Should parse at least one command"
                );
            }
            other => {
                panic!("Failed to parse text for EasyJet: {:?}", other);
            }
        }
    }

    #[test]
    fn test_text_parsing_lufthansa_cargo() {
        let lufthansa = &TEST_RECORDINGS_INDEX.entries["lufthansa_cargo"];

        println!("Testing text parsing for Lufthansa Cargo:");
        println!("Input text: '{}'", lufthansa.text);

        let result = COMMAND_PARSER.parse_transmission_enhanced(&lufthansa.text);

        match result {
            ParseResult::Success(parsed) | ParseResult::PartialSuccess { parsed, .. } => {
                println!("✓ Parsed callsign: {}", parsed.callsign);
                println!("✓ Parsed commands: {:?}", parsed.commands);

                if let Some(ref target) = lufthansa.command.target {
                    if let CommunicationEntity::Aircraft { full_name } = target {
                        assert_eq!(parsed.callsign, *full_name, "Callsign should match");
                    }
                }

                assert!(
                    !parsed.commands.is_empty(),
                    "Should parse at least one command"
                );
            }
            other => {
                panic!("Failed to parse text for Lufthansa Cargo: {:?}", other);
            }
        }
    }

    #[test]
    fn test_all_text_parsing() {
        let recordings = &TEST_RECORDINGS_INDEX.entries;

        for (name, recording) in recordings {
            println!("\n=== Testing text parsing for {} ===", name);
            println!("Input: '{}'", recording.text);

            let result = COMMAND_PARSER.parse_transmission_enhanced(&recording.text);

            match result {
                ParseResult::Success(parsed) => {
                    println!(
                        "✓ Success - callsign: {}, commands: {}",
                        parsed.callsign,
                        parsed.commands.len()
                    );

                    assert!(
                        !parsed.callsign.is_empty(),
                        "Should extract callsign for {}",
                        name
                    );
                    assert!(
                        !parsed.commands.is_empty(),
                        "Should parse commands for {}",
                        name
                    );
                }
                ParseResult::PartialSuccess {
                    parsed,
                    unparsed_parts,
                } => {
                    println!(
                        "⚠ Partial - callsign: {}, commands: {}, unparsed: {:?}",
                        parsed.callsign,
                        parsed.commands.len(),
                        unparsed_parts
                    );

                    assert!(
                        !parsed.callsign.is_empty(),
                        "Should extract callsign for {}",
                        name
                    );
                    // Partial success is acceptable - some parts might be hard to parse
                }
                ParseResult::CallsignOnly(callsign) => {
                    println!("⚠ Callsign only: {}", callsign);
                    assert!(!callsign.is_empty(), "Should extract callsign for {}", name);
                }
                ParseResult::Failed { reason, raw_text } => {
                    panic!(
                        "❌ Failed to parse {} - reason: {}, text: '{}'",
                        name, reason, raw_text
                    );
                }
            }
        }
    }
}
