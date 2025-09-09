use atc_recognition_rs::graph_parser::{GraphParser, ParserConfig};
use aviation_helper_rs::clearance::airlines::Airlines;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from RON file
    let config = match ParserConfig::load_default() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load parser config: {}", e);
            eprintln!("Make sure the file 'resources/parser/parser_config.ron' exists.");
            return Err(e);
        }
    };
    
    // Load airlines database
    let airlines = match Airlines::load_airlines_from_file() {
        Ok(airlines) => airlines,
        Err(e) => {
            eprintln!("Failed to load airlines database: {}", e);
            return Err(e.into());
        }
    };

    // Create the graph parser
    let parser = GraphParser::new(config, &airlines);

    // Test with some example commands
    let test_commands = vec![
        "american 123 turn left heading 270",
        "delta 456 climb flight level 350",
        "united seven eight nine descend altitude 8000 feet",
        "lufthansa 234 contact tower 120.9",
        "hedding two seven zero", // Test fuzzy matching for "heading"
    ];

    println!("Testing GraphParser with example commands:\n");

    for command in test_commands {
        println!("Input: \"{}\"", command);
        let result = parser.parse(command);
        
        match result {
            atc_recognition_rs::graph_parser::ParseResult::Success(parsed) => {
                println!("  ✓ Success: callsign={} (confidence: {:.2})", 
                         parsed.callsign, parsed.callsign_confidence);
                for cmd in &parsed.commands {
                    println!("    Command: {:?} (confidence: {:.2})", 
                             cmd.command, cmd.confidence);
                }
            }
            atc_recognition_rs::graph_parser::ParseResult::PartialSuccess { parsed, unparsed_parts } => {
                println!("  ~ Partial: callsign={} (confidence: {:.2})", 
                         parsed.callsign, parsed.callsign_confidence);
                for cmd in &parsed.commands {
                    println!("    Command: {:?} (confidence: {:.2})", 
                             cmd.command, cmd.confidence);
                }
                println!("    Unparsed: {:?}", unparsed_parts);
            }
            atc_recognition_rs::graph_parser::ParseResult::CallsignOnly(callsign) => {
                println!("  ? Callsign only: {}", callsign);
            }
            atc_recognition_rs::graph_parser::ParseResult::Failed { reason, .. } => {
                println!("  ✗ Failed: {}", reason);
            }
        }
        println!();
    }

    Ok(())
}
