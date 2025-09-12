//! Example: Continuous Voice Recognition for Aviation Commands
//!
//! This example demonstrates how to use the VoiceRecognizer for continuous
//! voice recognition of aviation commands.

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use atc_recognition_rs::{Error, SpeechToText, SpeechToTextConfig};
use aviation_helper_rs::{
    clearance::{airlines::Airlines, aviation_command::AviationCommandPart},
    types::altitude::Altitude,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛩️  ATC Voice Recognition - Continuous Mode Demo");
    println!("==============================================\n");

    // Create configuration for voice recognition
    let config = SpeechToTextConfig::default();

    // Load airlines database
    let airlines = Airlines::load_airlines_from_file().expect("Failed to load airlines database");

    // Create voice recognizer
    let recognizer = VoiceRecognizer::new(config, airlines)?;

    println!("✅ Voice recognizer initialized");
    println!(
        "   - Speech-to-text model available: {}",
        recognizer.speech_to_text().is_model_available()
    );
    println!(
        "   - Expected sample rate: {}Hz",
        recognizer.speech_to_text().sample_rate()
    );

    // Choose which example to run
    println!("\nSelect demo mode:");
    println!("1. Basic continuous recognition");
    println!("2. Advanced recognition with logging");
    println!("3. Interactive mode with single commands");

    let choice = get_user_choice()?;

    match choice {
        1 => run_basic_continuous_recognition(recognizer)?,
        2 => run_advanced_continuous_recognition(recognizer)?,
        3 => run_interactive_single_commands(recognizer)?,
        _ => {
            println!("Invalid choice. Running basic continuous recognition...");
            run_basic_continuous_recognition(recognizer)?;
        }
    }

    Ok(())
}

/// Basic continuous voice recognition example
fn run_basic_continuous_recognition(
    recognizer: VoiceRecognizer,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎙️  Starting BASIC continuous voice recognition...");
    println!("Speak aviation commands like:");
    println!("  - 'Turn left heading two seven zero'");
    println!("  - 'Climb to flight level three five zero'");
    println!("  - 'Contact tower on one two one point five'");
    println!("\nPress Ctrl+C to stop...\n");

    // Simple callback that just prints recognized commands
    let callback = |command: AviationCommandPart| match command {
        AviationCommandPart::FlyHeading {
            heading,
            turn_direction,
        } => {
            println!(
                "🛩️  Heading Command: fly heading {:?} (turn: {:?})",
                heading, turn_direction
            );
        }
        AviationCommandPart::TurnBy {
            degrees,
            turn_direction,
        } => {
            println!(
                "🔄 Turn Command: turn {:?} by {:?} degrees",
                turn_direction, degrees
            );
        }
        AviationCommandPart::ChangeAltitude {
            altitude,
            turn_direction,
            ..
        } => {
            println!(
                "📏 Altitude Command: {:?} to {}",
                turn_direction,
                format_altitude(&altitude)
            );
        }
        AviationCommandPart::ContactFrequency { frequency, station } => {
            let station_str = station.as_deref().unwrap_or("ATC");
            println!(
                "📡 Contact Command: contact {} on {:.2}",
                station_str, frequency.num
            );
        }
        AviationCommandPart::ProceedDirect(waypoint) => {
            println!("🎯 Navigation Command: proceed direct {}", waypoint);
        }
        AviationCommandPart::RadarContact => {
            println!("📡 Radar Command: radar contact");
        }
    };

    // Start continuous recognition (this blocks)
    recognizer.start_continuous_recognition(callback)?;

    Ok(())
}

/// Advanced continuous recognition with command logging
fn run_advanced_continuous_recognition(
    recognizer: VoiceRecognizer,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎙️  Starting ADVANCED continuous voice recognition...");
    println!("This demo includes command logging and timestamps.");
    println!("\nPress Ctrl+C to stop...\n");

    // Shared log for commands
    let command_log = Arc::new(Mutex::new(Vec::<(Instant, AviationCommandPart)>::new()));
    let log_clone = command_log.clone();

    // Advanced callback with logging
    let callback = move |command: AviationCommandPart| {
        let timestamp = Instant::now();

        // Log the command
        {
            let mut log = log_clone.lock().unwrap();
            log.push((timestamp, command.clone()));
        }

        // Print with timestamp
        println!(
            "[{}] 🎯 Command recognized:",
            log_clone.lock().unwrap().len()
        );

        match command {
            AviationCommandPart::FlyHeading {
                heading,
                turn_direction,
            } => {
                println!(
                    "   🛩️  HEADING: fly heading {:?} (turn: {:?})",
                    heading, turn_direction
                );
            }
            AviationCommandPart::TurnBy {
                degrees,
                turn_direction,
            } => {
                println!(
                    "   🔄 TURN: turn {:?} by {:?} degrees",
                    turn_direction, degrees
                );
            }
            AviationCommandPart::ChangeAltitude {
                altitude,
                turn_direction,
                ..
            } => {
                println!(
                    "   📏 ALTITUDE: {:?} to {}",
                    turn_direction,
                    format_altitude(&altitude)
                );
            }
            AviationCommandPart::ContactFrequency { frequency, station } => {
                let station_str = station.as_deref().unwrap_or("ATC");
                println!("   📡 CONTACT: {} on {:.2}", station_str, frequency.num);
            }
            AviationCommandPart::ProceedDirect(waypoint) => {
                println!("   🎯 NAVIGATE: proceed direct {}", waypoint);
            }
            AviationCommandPart::RadarContact => {
                println!("   📡 RADAR: radar contact");
            }
        }

        // Print log summary every 5 commands
        let log = log_clone.lock().unwrap();
        if log.len() % 5 == 0 {
            println!("\n📊 Summary: {} commands recognized so far", log.len());
        }
    };

    // Start a monitoring thread to print periodic summaries
    let log_monitor = command_log.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(30));
            let log = log_monitor.lock().unwrap();
            if !log.is_empty() {
                println!("\n⏰ Periodic Update: {} total commands in log", log.len());
            }
        }
    });

    // Start continuous recognition
    recognizer.start_continuous_recognition(callback)?;

    Ok(())
}

/// Interactive single command mode
fn run_interactive_single_commands(
    recognizer: VoiceRecognizer,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎙️  Interactive Single Command Mode");
    println!("Press Enter to capture and recognize a single command, 'q' to quit.\n");

    loop {
        println!("Press Enter to start recording (3 seconds)...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() == "q" {
            break;
        }

        println!("🔴 Recording... (3 seconds)");

        match recognizer.recognize_single_command_live(3.0) {
            Ok(Some(command)) => {
                println!("✅ Recognized command: {:?}", command);
                match command {
                    AviationCommandPart::FlyHeading {
                        heading,
                        turn_direction,
                    } => {
                        println!(
                            "   → Aircraft should fly heading {:?} (turn: {:?})",
                            heading, turn_direction
                        );
                    }
                    AviationCommandPart::TurnBy {
                        degrees,
                        turn_direction,
                    } => {
                        println!(
                            "   → Aircraft should turn {:?} by {:?} degrees",
                            turn_direction, degrees
                        );
                    }
                    AviationCommandPart::ChangeAltitude {
                        altitude,
                        turn_direction,
                        ..
                    } => {
                        println!(
                            "   → Aircraft should change altitude {:?} to {}",
                            turn_direction,
                            format_altitude(&altitude)
                        );
                    }
                    AviationCommandPart::ContactFrequency { frequency, station } => {
                        let station_str = station.as_deref().unwrap_or("ATC");
                        println!(
                            "   → Aircraft should contact {} on {:.2}",
                            station_str, frequency.num
                        );
                    }
                    AviationCommandPart::ProceedDirect(waypoint) => {
                        println!("   → Aircraft should proceed direct {}", waypoint);
                    }
                    AviationCommandPart::RadarContact => {
                        println!("   → Radar contact established");
                    }
                }
            }
            Ok(None) => {
                println!("❌ No command recognized");
            }
            Err(e) => {
                println!("❌ Error: {:?}", e);
            }
        }

        println!();
    }

    println!("� Goodbye!");
    Ok(())
}

// Helper functions

fn format_altitude(altitude: &Altitude) -> String {
    match altitude {
        Altitude::Feet(ft) => format!("{} feet", ft),
        Altitude::FlightLevel(fl) => format!("FL{}", fl),
    }
}

fn get_user_choice() -> Result<u32, Box<dyn std::error::Error>> {
    print!("Enter choice (1-3): ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    Ok(input.trim().parse().unwrap_or(1))
}

/// Capture audio from microphone for a specified duration and return the samples
/// This is the foundation for real single-command recognition
fn capture_audio_samples(
    speech_to_text: &SpeechToText,
    duration_seconds: f32,
) -> Result<Vec<f32>, Error> {
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
