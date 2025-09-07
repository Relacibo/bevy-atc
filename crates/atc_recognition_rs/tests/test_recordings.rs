//! Tests for real audio recording parsing
//!
//! This module tests the parser against real ATC audio recordings using the RON index file
//! to verify that our parser can handle actual speech-to-text output.

use atc_recognition_rs::{
    SpeechToText, SpeechToTextConfig, VoiceRecognizer,
    parser::{AviationCommandParser, ParseResult},
};
use aviation_helper_rs::clearance::{
    airlines::Airlines,
    aviation_command::{AviationCommandGroup, CommunicationEntity},
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

static AIRLINES: LazyLock<Airlines> =
    LazyLock::new(|| Airlines::load_airlines_from_file().unwrap());

static COMMAND_PARSER: LazyLock<AviationCommandParser> =
    LazyLock::new(|| AviationCommandParser::new(AIRLINES.clone()));

static TEST_RECORDINGS_INDEX: LazyLock<TestRecordingsIndex> =
    LazyLock::new(TestRecordingsIndex::load_from_file);

static SPEECH_TO_TEXT: LazyLock<SpeechToText> = LazyLock::new(|| {
    let config = SpeechToTextConfig::default();
    SpeechToText::new(config).expect("Failed to create SpeechToText object!")
});

fn resolve_test_recording_file(file_name: &str) -> PathBuf {
    format!(
        "{}/resources/test-recordings/{}",
        { env!("CARGO_MANIFEST_DIR") },
        file_name
    )
    .into()
}

#[derive(Clone, Debug, Deserialize)]
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

#[cfg(test)]
mod audio_recognition_tests {
    use atc_recognition_rs::test_utils;

    use super::*;

    fn test_audio_recognition_for_aircraft(aircraft_key: &str) {
        let recording = &TEST_RECORDINGS_INDEX.entries[aircraft_key];
        let wav_path = resolve_test_recording_file(&recording.file_name);

        let transcribed_text = test_utils::process_wav_file_for_test(&SPEECH_TO_TEXT, wav_path)
            .inspect_err(|err| eprintln!("{err:?}"))
            .expect("WAV file processing must succeed");

        println!("✓ Successfully processed WAV file");
        println!("Transcribed: '{}'", transcribed_text);

        assert_eq!(transcribed_text, recording.text);
    }

    #[test]
    fn test_audio_recognition_delta() {
        test_audio_recognition_for_aircraft("delta");
    }

    #[test]
    fn test_audio_recognition_easyjet() {
        test_audio_recognition_for_aircraft("easyjet");
    }

    #[test]
    fn test_audio_recognition_lufthansa_cargo() {
        test_audio_recognition_for_aircraft("lufthansa_cargo");
    }
}

/// Tests for direct text-to-command parsing (no audio involved)
#[cfg(test)]
mod text_parsing_tests {
    use super::*;

    fn test_text_parsing_for_aircraft(aircraft_key: &str) {
        let TestRecording {
            command: expected_command,
            text: expected_text,
            ..
        } = &TEST_RECORDINGS_INDEX.entries[aircraft_key];

        let AviationCommandGroup {
            target: expected_target,
            parts: expected_command_parts,
        } = expected_command;

        println!("Testing text parsing for {}:", aircraft_key);
        println!("Input text: '{}'", expected_text);
        println!("Expected: {:?}", expected_command);

        let result = COMMAND_PARSER.parse_transmission_enhanced(expected_text);

        match result {
            ParseResult::Success(parsed) => {
                println!("✓ Parsed callsign: {}", parsed.callsign);
                println!("✓ Parsed commands: {:?}", parsed.commands);

                // Verify callsign
                let Some(target) = expected_target else {
                    panic!("No call sign!");
                };

                if let CommunicationEntity::Aircraft { full_name } = target {
                    assert_eq!(parsed.callsign, *full_name, "Callsign should match");
                }

                // Verify we parsed some commands
                assert_eq!(
                    &parsed
                        .commands
                        .into_iter()
                        .map(|c| c.command)
                        .collect::<Vec<_>>(),
                    expected_command_parts,
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
                assert_eq!(
                    &Some(CommunicationEntity::Aircraft {
                        full_name: parsed.callsign
                    }),
                    expected_target,
                    "Callsign doesn't match!"
                );
                panic!("Failed successful parse for {}!", aircraft_key);
            }
            other => {
                panic!("Failed to parse text for {}: {:?}", aircraft_key, other);
            }
        }
    }

    #[test]
    fn test_parse_delta_text() {
        test_text_parsing_for_aircraft("delta");
    }

    #[test]
    fn test_parse_easyjet_text() {
        test_text_parsing_for_aircraft("easyjet");
    }

    #[test]
    fn test_parsing_lufthansa_cargo_text() {
        test_text_parsing_for_aircraft("lufthansa_cargo");
    }
}
