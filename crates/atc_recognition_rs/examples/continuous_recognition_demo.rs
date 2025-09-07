//! Example: Continuous Voice Recognition for Aviation Commands
//!
//! This example demonstrates how to use the VoiceRecognizer for continuous
//! voice recognition of aviation commands.

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use atc_recognition_rs::{RecognitionConfig, VoiceRecognizer};
use aviation_helper_rs::{
    clearance::{airlines::Airlines, aviation_command::AviationCommandPart},
    types::altitude::Altitude,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🛩️  ATC Voice Recognition - Continuous Mode Demo");
    println!("==============================================\n");

    // Create configuration for voice recognition
    let config = RecognitionConfig::default();

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
