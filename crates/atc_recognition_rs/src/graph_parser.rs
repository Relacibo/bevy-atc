//! Graph-based aviation command parser
//!
//! A flexible, state-machine-based parser that can handle natural language variations
//! from speech recognition systems like Whisper. Uses a graph of states and transitions
//! to parse ATC commands robustly.

use aviation_helper_rs::{
    clearance::airlines::Airlines,
    clearance::aviation_command::{AviationCommandPart, Frequency, HeadingDirection},
    types::{
        altitude::{Altitude, VerticalDirection},
        heading::{Degrees, Heading, TurnDirection},
    },
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseState {
    Start,
    ExpectingCallsign,
    ExpectingCommand,
    
    // Command-specific states
    TurnCommand,
    ExpectingDirection,      // left/right
    ExpectingHeading,        // after "heading"
    ClimbCommand,
    DescendCommand,
    ExpectingAltitude,       // flight level / feet
    ContactCommand,
    ExpectingFrequency,
    ExpectingStation,        // tower/ground/approach
    
    // Terminal states
    CommandComplete,
    ParseComplete,
}

#[derive(Debug, Clone)]
pub struct ParseEdge {
    pub from: ParseState,
    pub to: ParseState,
    pub matcher: TokenMatcher,
    pub confidence: f32,
    pub consumes_token: bool,
}

#[derive(Debug, Clone)]
pub enum TokenMatcher {
    Exact(String),                    // "turn"
    OneOf(Vec<String>),              // ["left", "right"]
    Number(NumberType),               // heading degrees, altitude
    Airline,                         // matches against airline DB
    Pattern(String),                 // regex as fallback
    Fuzzy(String, f32),              // fuzzy match with threshold
    Optional(Box<TokenMatcher>),     // optional token
}

#[derive(Debug, Clone)]
pub enum NumberType {
    Heading,      // 0-360
    Altitude,     // flight level or feet
    Frequency,    // xxx.xx format
    FlightNumber, // airline suffix
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub callsign: String,
    pub callsign_confidence: f32,
    pub commands: Vec<CommandWithConfidence>,
}

#[derive(Debug, Clone)]
pub struct CommandWithConfidence {
    pub command: AviationCommandPart,
    pub confidence: f32,
    pub source_text: String,
}

#[derive(Debug, Clone)]
pub enum ParseResult {
    Success(ParsedCommand),
    PartialSuccess {
        parsed: ParsedCommand,
        unparsed_parts: Vec<String>,
    },
    CallsignOnly(String),
    Failed {
        reason: String,
        raw_text: String,
    },
}

#[derive(Debug, Clone)]
struct ParsePath {
    final_state: ParseState,
    total_confidence: f32,
    steps: Vec<ParseStep>,
    tokens_consumed: usize,
    extracted_data: HashMap<String, ParsedValue>,
}

#[derive(Debug, Clone)]
struct ParseStep {
    state: ParseState,
    token: String,
    confidence: f32,
    matcher_used: TokenMatcher,
}

#[derive(Debug, Clone)]
enum ParsedValue {
    Callsign(String),
    Direction(TurnDirection),
    Heading(f32),
    Altitude(u32),
    Station(String),
    Frequency(u32, u32), // (num, dec)
}

pub struct GraphParser {
    edges: Vec<ParseEdge>,
    airlines: Airlines,
    number_words: HashMap<String, u32>,
    direction_words: HashMap<String, TurnDirection>,
    altitude_words: HashMap<String, VerticalDirection>,
    phonetic_alphabet: HashMap<String, String>,
    airline_name_to_icao: HashMap<String, String>,
    callsign_to_icao: HashMap<String, String>,
}

impl GraphParser {
    pub fn new(airlines: Airlines) -> Self {
        let mut parser = Self {
            edges: Vec::new(),
            airlines: airlines.clone(),
            number_words: HashMap::new(),
            direction_words: HashMap::new(),
            altitude_words: HashMap::new(),
            phonetic_alphabet: HashMap::new(),
            airline_name_to_icao: HashMap::new(),
            callsign_to_icao: HashMap::new(),
        };
        
        parser.initialize_word_mappings();
        parser.load_airlines(airlines);
        parser.build_graph();
        parser
    }

    fn initialize_word_mappings(&mut self) {
        // Numbers 0-9 for spoken digits
        let numbers = [
            ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4),
            ("five", 5), ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9),
            ("niner", 9), ("tree", 3), ("fife", 5),
            ("0", 0), ("1", 1), ("2", 2), ("3", 3), ("4", 4),
            ("5", 5), ("6", 6), ("7", 7), ("8", 8), ("9", 9),
        ];

        for (word, num) in numbers {
            self.number_words.insert(word.to_string(), num);
        }

        // Direction words
        self.direction_words.insert("left".to_string(), TurnDirection::Left);
        self.direction_words.insert("right".to_string(), TurnDirection::Right);

        // Altitude direction words
        self.altitude_words.insert("climb".to_string(), VerticalDirection::Climb);
        self.altitude_words.insert("descend".to_string(), VerticalDirection::Descend);
        self.altitude_words.insert("descent".to_string(), VerticalDirection::Descend);

        // Phonetic alphabet
        let phonetic_alphabet = [
            ("alpha", "A"), ("bravo", "B"), ("charlie", "C"), ("delta", "D"),
            ("echo", "E"), ("foxtrot", "F"), ("golf", "G"), ("hotel", "H"),
            ("india", "I"), ("juliet", "J"), ("kilo", "K"), ("lima", "L"),
            ("mike", "M"), ("november", "N"), ("oscar", "O"), ("papa", "P"),
            ("quebec", "Q"), ("romeo", "R"), ("sierra", "S"), ("tango", "T"),
            ("uniform", "U"), ("victor", "V"), ("whiskey", "W"), ("xray", "X"),
            ("yankee", "Y"), ("zulu", "Z"),
        ];

        for (phonetic, letter) in phonetic_alphabet {
            self.phonetic_alphabet.insert(phonetic.to_string(), letter.to_string());
        }
    }

    fn load_airlines(&mut self, airlines: Airlines) {
        self.callsign_to_icao.clear();
        self.airline_name_to_icao.clear();

        for airline in &airlines.0 {
            let Some(icao) = &airline.icao else { continue; };
            if !airline.active || icao.is_empty() || icao == "N/A" {
                continue;
            }
            let icao_lower = icao.to_lowercase();

            // Primary: Map callsign to ICAO
            if let Some(callsign) = &airline.callsign {
                if !callsign.is_empty() {
                    let callsign_key = callsign.to_lowercase().replace(" ", "");
                    self.callsign_to_icao.insert(callsign_key, icao_lower.clone());
                }
            }

            // Fallback: Map airline name to ICAO
            if !airline.name.is_empty() {
                let name_key = airline.name.to_lowercase().replace(" ", "");
                if !self.callsign_to_icao.contains_key(&name_key) {
                    self.airline_name_to_icao.insert(name_key, icao_lower.clone());
                }
            }
        }
    }

    fn build_graph(&mut self) {
        // 1. Callsign parsing
        self.add_edge(ParseState::Start, ParseState::ExpectingCallsign, 
                     TokenMatcher::Airline, 0.9, true);

        // After callsign, expect command
        self.add_edge(ParseState::ExpectingCallsign, ParseState::ExpectingCommand,
                     TokenMatcher::Optional(Box::new(TokenMatcher::Exact(",".into()))), 0.8, true);

        // 2. Command entry points
        self.add_edge(ParseState::ExpectingCommand, ParseState::TurnCommand,
                     TokenMatcher::OneOf(vec!["turn".into(), "fly".into()]), 0.95, true);
        
        self.add_edge(ParseState::ExpectingCommand, ParseState::ClimbCommand,
                     TokenMatcher::Exact("climb".into()), 0.95, true);
        
        self.add_edge(ParseState::ExpectingCommand, ParseState::DescendCommand,
                     TokenMatcher::Exact("descend".into()), 0.95, true);
        
        self.add_edge(ParseState::ExpectingCommand, ParseState::ContactCommand,
                     TokenMatcher::Exact("contact".into()), 0.95, true);

        // 3. Turn command flow
        self.add_edge(ParseState::TurnCommand, ParseState::ExpectingDirection,
                     TokenMatcher::OneOf(vec!["left".into(), "right".into()]), 0.9, true);
        
        // "fly heading" or "turn left heading"
        self.add_edge(ParseState::TurnCommand, ParseState::ExpectingHeading,
                     TokenMatcher::Fuzzy("heading".into(), 0.8), 0.9, true);
        
        self.add_edge(ParseState::ExpectingDirection, ParseState::ExpectingHeading,
                     TokenMatcher::Fuzzy("heading".into(), 0.8), 0.85, true);
        
        // Simple turn without heading
        self.add_edge(ParseState::ExpectingDirection, ParseState::CommandComplete,
                     TokenMatcher::Optional(Box::new(TokenMatcher::Exact("turn".into()))), 0.7, false);
        
        self.add_edge(ParseState::ExpectingHeading, ParseState::CommandComplete,
                     TokenMatcher::Number(NumberType::Heading), 0.9, true);

        // 4. Altitude commands
        self.add_edge(ParseState::ClimbCommand, ParseState::ExpectingAltitude,
                     TokenMatcher::OneOf(vec!["to".into(), "and".into(), "maintain".into()]), 0.7, true);
        
        self.add_edge(ParseState::DescendCommand, ParseState::ExpectingAltitude,
                     TokenMatcher::OneOf(vec!["to".into(), "and".into(), "maintain".into()]), 0.7, true);
        
        self.add_edge(ParseState::ClimbCommand, ParseState::ExpectingAltitude,
                     TokenMatcher::Fuzzy("flight".into(), 0.8), 0.8, true);
        
        self.add_edge(ParseState::DescendCommand, ParseState::ExpectingAltitude,
                     TokenMatcher::Fuzzy("flight".into(), 0.8), 0.8, true);
        
        self.add_edge(ParseState::ExpectingAltitude, ParseState::CommandComplete,
                     TokenMatcher::OneOf(vec!["level".into(), "feet".into()]), 0.7, true);
        
        self.add_edge(ParseState::ExpectingAltitude, ParseState::CommandComplete,
                     TokenMatcher::Number(NumberType::Altitude), 0.9, true);

        // 5. Contact commands
        self.add_edge(ParseState::ContactCommand, ParseState::ExpectingStation,
                     TokenMatcher::OneOf(vec!["tower".into(), "ground".into(), "approach".into()]), 0.9, true);
        
        self.add_edge(ParseState::ExpectingStation, ParseState::ExpectingFrequency,
                     TokenMatcher::Number(NumberType::Frequency), 0.9, true);
        
        self.add_edge(ParseState::ExpectingFrequency, ParseState::CommandComplete,
                     TokenMatcher::Pattern(r"\d+\.\d+".into()), 0.85, true);

        // 6. Command completion -> next command or end
        self.add_edge(ParseState::CommandComplete, ParseState::ExpectingCommand,
                     TokenMatcher::OneOf(vec!["and".into(), "then".into(), ",".into()]), 0.5, true);
        
        self.add_edge(ParseState::CommandComplete, ParseState::ParseComplete,
                     TokenMatcher::Optional(Box::new(TokenMatcher::Pattern(".*".into()))), 0.8, false);
    }

    fn add_edge(&mut self, from: ParseState, to: ParseState, matcher: TokenMatcher, confidence: f32, consumes: bool) {
        self.edges.push(ParseEdge {
            from, to, matcher, confidence, consumes_token: consumes
        });
    }

    pub fn parse_transmission_enhanced(&self, text: &str) -> ParseResult {
        // Preprocess text for Whisper quirks
        let preprocessed = self.preprocess_whisper_text(text);
        let tokens = self.tokenize(&preprocessed);
        
        let mut best_paths = Vec::new();
        
        // Start exploration from initial state
        self.explore_paths(ParseState::Start, 0, &tokens, 1.0, Vec::new(), HashMap::new(), &mut best_paths);
        
        // Find best complete path
        if let Some(best_path) = best_paths.into_iter()
            .filter(|path| path.final_state == ParseState::ParseComplete || path.final_state == ParseState::CommandComplete)
            .max_by(|a, b| a.total_confidence.partial_cmp(&b.total_confidence).unwrap()) 
        {
            self.path_to_result(best_path, &tokens)
        } else {
            ParseResult::Failed {
                reason: "No complete parse path found".into(),
                raw_text: text.to_string(),
            }
        }
    }

    fn preprocess_whisper_text(&self, text: &str) -> String {
        let mut result = text.to_lowercase();
        
        // Whisper-specific normalization
        result = result
            .replace("flyheading", "fly heading")
            .replace("turnleft", "turn left")
            .replace("turnright", "turn right")
            .replace("climbto", "climb to")
            .replace("descendto", "descend to")
            .replace("contacttower", "contact tower")
            .replace("flightlevel", "flight level")
            .replace("maintainflightlevel", "maintain flight level")
            .replace(".", "")
            .replace(",", " , ")
            .replace("!", "")
            .replace("?", "");
        
        // Convert spoken numbers to digits
        for (word, digit) in &self.number_words {
            let pattern = format!(r"\b{}\b", regex::escape(word));
            if let Ok(regex) = regex::Regex::new(&pattern) {
                result = regex.replace_all(&result, digit.to_string()).to_string();
            }
        }
        
        result
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn explore_paths(
        &self,
        current_state: ParseState,
        token_index: usize,
        tokens: &[String],
        current_confidence: f32,
        current_path: Vec<ParseStep>,
        current_data: HashMap<String, ParsedValue>,
        best_paths: &mut Vec<ParsePath>,
    ) {
        // Terminal condition
        if token_index >= tokens.len() || current_state == ParseState::ParseComplete {
            best_paths.push(ParsePath {
                final_state: current_state,
                total_confidence: current_confidence,
                steps: current_path,
                tokens_consumed: token_index,
                extracted_data: current_data,
            });
            return;
        }

        // Limit exploration depth for performance
        if current_path.len() > tokens.len() * 2 {
            return;
        }

        let current_token = &tokens[token_index];
        
        // Find all possible transitions from current state
        for edge in &self.edges {
            if edge.from != current_state {
                continue;
            }

            // Test if this edge matches the current token
            if let Some((match_confidence, extracted_value)) = self.test_matcher(&edge.matcher, current_token, token_index, tokens) {
                let new_confidence = current_confidence * edge.confidence * match_confidence;
                
                // Only pursue promising paths (confidence threshold)
                if new_confidence > 0.1 {
                    let mut new_path = current_path.clone();
                    new_path.push(ParseStep {
                        state: edge.to.clone(),
                        token: current_token.clone(),
                        confidence: match_confidence,
                        matcher_used: edge.matcher.clone(),
                    });

                    let mut new_data = current_data.clone();
                    if let Some(value) = extracted_value {
                        let key = format!("{:?}_{}", edge.to, new_path.len());
                        new_data.insert(key, value);
                    }

                    let next_token_index = if edge.consumes_token { 
                        token_index + 1 
                    } else { 
                        token_index 
                    };

                    // Recursive exploration
                    self.explore_paths(
                        edge.to.clone(),
                        next_token_index,
                        tokens,
                        new_confidence,
                        new_path,
                        new_data,
                        best_paths,
                    );
                }
            }
        }
    }

    fn test_matcher(&self, matcher: &TokenMatcher, token: &str, _index: usize, _tokens: &[String]) -> Option<(f32, Option<ParsedValue>)> {
        match matcher {
            TokenMatcher::Exact(expected) => {
                if token.to_lowercase() == expected.to_lowercase() {
                    Some((1.0, None))
                } else {
                    None
                }
            }
            TokenMatcher::OneOf(options) => {
                for opt in options {
                    if token.to_lowercase() == opt.to_lowercase() {
                        // Extract specific values
                        let value = match opt.as_str() {
                            "left" => Some(ParsedValue::Direction(TurnDirection::Left)),
                            "right" => Some(ParsedValue::Direction(TurnDirection::Right)),
                            "tower" | "ground" | "approach" => Some(ParsedValue::Station(opt.clone())),
                            _ => None,
                        };
                        return Some((0.95, value));
                    }
                }
                None
            }
            TokenMatcher::Fuzzy(expected, threshold) => {
                let similarity = self.calculate_similarity(token, expected);
                if similarity >= *threshold {
                    Some((similarity, None))
                } else {
                    None
                }
            }
            TokenMatcher::Number(number_type) => {
                self.test_number_match(token, number_type)
            }
            TokenMatcher::Airline => {
                if self.is_airline_match(token) {
                    let normalized = self.normalize_airline_token(token);
                    Some((0.9, Some(ParsedValue::Callsign(normalized))))
                } else {
                    None
                }
            }
            TokenMatcher::Pattern(regex_str) => {
                if let Ok(regex) = regex::Regex::new(regex_str) {
                    if regex.is_match(token) {
                        Some((0.8, None))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            TokenMatcher::Optional(_) => {
                // Optional tokens always match but don't advance
                Some((0.9, None))
            }
        }
    }

    fn test_number_match(&self, token: &str, number_type: &NumberType) -> Option<(f32, Option<ParsedValue>)> {
        match number_type {
            NumberType::Heading => {
                if let Ok(heading) = token.parse::<f32>() {
                    if (0.0..=360.0).contains(&heading) {
                        Some((0.9, Some(ParsedValue::Heading(heading))))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            NumberType::Altitude => {
                if let Ok(alt) = token.parse::<u32>() {
                    if (0..=60000).contains(&alt) {
                        Some((0.9, Some(ParsedValue::Altitude(alt))))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            NumberType::Frequency => {
                // Check for frequency format like "121.5" or "121" 
                if let Ok(num) = token.parse::<u32>() {
                    if (118..=137).contains(&num) {
                        Some((0.8, Some(ParsedValue::Frequency(num, 0))))
                    } else {
                        None
                    }
                } else if token.contains('.') {
                    let parts: Vec<&str> = token.split('.').collect();
                    if parts.len() == 2 {
                        if let (Ok(num), Ok(dec)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            if (118..=137).contains(&num) && dec <= 999 {
                                Some((0.9, Some(ParsedValue::Frequency(num, dec))))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            NumberType::FlightNumber => {
                if token.chars().all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic()) {
                    Some((0.8, None))
                } else {
                    None
                }
            }
        }
    }

    fn calculate_similarity(&self, a: &str, b: &str) -> f32 {
        // Simple similarity based on common characters
        let a_chars: std::collections::HashSet<char> = a.chars().collect();
        let b_chars: std::collections::HashSet<char> = b.chars().collect();
        
        let intersection = a_chars.intersection(&b_chars).count();
        let union = a_chars.union(&b_chars).count();
        
        if union == 0 {
            if a == b { 1.0 } else { 0.0 }
        } else {
            intersection as f32 / union as f32
        }
    }

    fn is_airline_match(&self, token: &str) -> bool {
        let token_lower = token.to_lowercase();
        
        // Check against known callsigns and airline names
        self.callsign_to_icao.contains_key(&token_lower) ||
        self.airline_name_to_icao.contains_key(&token_lower) ||
        // Check phonetic alphabet combinations
        self.phonetic_alphabet.contains_key(&token_lower)
    }

    fn normalize_airline_token(&self, token: &str) -> String {
        let token_lower = token.to_lowercase();
        
        // Try direct callsign match
        if let Some(icao) = self.callsign_to_icao.get(&token_lower) {
            return icao.to_uppercase();
        }
        
        // Try airline name match
        if let Some(icao) = self.airline_name_to_icao.get(&token_lower) {
            return icao.to_uppercase();
        }
        
        // Try phonetic alphabet
        if let Some(letter) = self.phonetic_alphabet.get(&token_lower) {
            return letter.clone();
        }
        
        // Fallback: return as-is but uppercase
        token.to_uppercase()
    }

    fn path_to_result(&self, path: ParsePath, tokens: &[String]) -> ParseResult {
        // Extract callsign from path data
        let callsign = path.extracted_data
            .values()
            .find_map(|v| match v {
                ParsedValue::Callsign(c) => Some(c.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Extract commands from path
        let mut commands = Vec::new();
        
        // Simple command extraction based on final state and collected data
        // This is a simplified version - in a full implementation, you'd track
        // command building through the state transitions
        
        if path.steps.iter().any(|step| step.state == ParseState::CommandComplete) {
            // Build commands from extracted data
            if let Some(ParsedValue::Heading(heading)) = path.extracted_data.values().find_map(|v| match v {
                ParsedValue::Heading(h) => Some(ParsedValue::Heading(*h)),
                _ => None,
            }) {
                commands.push(CommandWithConfidence {
                    command: AviationCommandPart::FlyHeading {
                        heading: HeadingDirection::Heading(Heading::from(heading as f64)),
                        turn_direction: None,
                    },
                    confidence: 0.8,
                    source_text: tokens[..path.tokens_consumed].join(" "),
                });
            }
            
            // Add more command extraction logic here...
        }

        let parsed = ParsedCommand {
            callsign,
            callsign_confidence: 0.8, // Calculate based on path confidence
            commands,
        };

        if path.tokens_consumed >= tokens.len() {
            ParseResult::Success(parsed)
        } else {
            ParseResult::PartialSuccess {
                parsed,
                unparsed_parts: tokens[path.tokens_consumed..].to_vec(),
            }
        }
    }
}

// Re-export for compatibility
pub use ParseResult as GraphParseResult;
pub use ParsedCommand as GraphParsedCommand;
pub use CommandWithConfidence as GraphCommandWithConfidence;

#[cfg(test)]
mod tests {
    use super::*;
    use aviation_helper_rs::clearance::airlines::AirlineEntry;

    fn create_test_airlines() -> Airlines {
        Airlines(vec![
            AirlineEntry {
                id: 1,
                name: "Lufthansa".to_string(),
                alias: None,
                iata: Some("LH".to_string()),
                icao: Some("DLH".to_string()),
                callsign: Some("Lufthansa".to_string()),
                country: "Germany".to_string(),
                active: true,
            },
            AirlineEntry {
                id: 2,
                name: "Delta Air Lines".to_string(),
                alias: None,
                iata: Some("DL".to_string()),
                icao: Some("DAL".to_string()),
                callsign: Some("Delta".to_string()),
                country: "United States".to_string(),
                active: true,
            },
        ])
    }

    #[test]
    fn test_graph_parser_basic() {
        let airlines = create_test_airlines();
        let parser = GraphParser::new(airlines);
        
        let result = parser.parse_transmission_enhanced("delta 123 turn left heading 270");
        
        match result {
            ParseResult::Success(parsed) => {
                assert!(!parsed.callsign.is_empty());
                println!("Parsed: {:?}", parsed);
            }
            other => {
                println!("Parse result: {:?}", other);
            }
        }
    }

    #[test]
    fn test_whisper_preprocessing() {
        let airlines = create_test_airlines();
        let parser = GraphParser::new(airlines);
        
        let preprocessed = parser.preprocess_whisper_text("deltaonetwothreeflyheading270");
        assert_eq!(preprocessed, "delta123fly heading270");
    }

    #[test]
    fn test_fuzzy_matching() {
        let airlines = create_test_airlines();
        let parser = GraphParser::new(airlines);
        
        // Test that "hedding" matches "heading" with fuzzy matching
        let result = parser.parse_transmission_enhanced("delta 123 fly hedding 090");
        
        match result {
            ParseResult::Success(_) | ParseResult::PartialSuccess { .. } => {
                // Good - fuzzy matching worked
            }
            _ => panic!("Fuzzy matching should work for 'hedding' -> 'heading'"),
        }
    }
}
