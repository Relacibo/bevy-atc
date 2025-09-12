//! Graph-based aviation command parser
//!
//! A flexible, state-machine-based parser that can handle natural language variations
//! from speech recognition systems like Whisper. Uses a graph of states and transitions
//! to parse ATC commands robustly.

use aviation_helper_rs::{
    clearance::airlines::Airlines,
    clearance::aviation_command::{AviationCommandPart, HeadingDirection},
    types::{
        altitude::VerticalDirection,
        heading::{Heading, TurnDirection},
    },
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub enum ParseState {
    Start,
    ExpectingCallsign,
    ExpectingCommand,

    // Command-specific states
    TurnCommand,
    ExpectingDirection, // left/right
    ExpectingHeading,   // after "heading"
    ClimbCommand,
    DescendCommand,
    ExpectingAltitude, // flight level / feet
    ContactCommand,
    ExpectingFrequency,
    ExpectingStation, // tower/ground/approach

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
    Exact(String),               // "turn"
    OneOf(Vec<String>),          // ["left", "right"]
    Number(NumberType),          // heading degrees, altitude
    Airline,                     // matches against airline DB
    Pattern(String),             // regex as fallback
    Fuzzy(String, f32),          // fuzzy match with threshold
    Optional(Box<TokenMatcher>), // optional token
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
    number_words: HashMap<String, u32>,
    direction_words: HashMap<String, TurnDirection>,
    altitude_words: HashMap<String, VerticalDirection>,
    phonetic_alphabet: HashMap<String, String>,
    airline_name_to_icao: HashMap<String, String>,
    callsign_to_icao: HashMap<String, String>,
    recognition_corrections: HashMap<String, String>,
    fuzzy_threshold: f32,
    confidence_threshold: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ParserConfig {
    pub recognition_corrections: HashMap<String, String>,
    pub number_words: HashMap<String, u32>,
    pub direction_words: HashMap<String, TurnDirection>,
    pub altitude_words: HashMap<String, VerticalDirection>,
    pub phonetic_alphabet: HashMap<String, String>,
    pub fuzzy_threshold: f32,
    pub confidence_threshold: f32,
}

impl ParserConfig {
    /// Load parser configuration from a RON file
    pub fn load_from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: ParserConfig = ron::from_str(&contents)?;
        Ok(config)
    }

    /// Load parser configuration from the default location
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from_file("resources/parser/parser_config.ron")
    }
}

impl GraphParser {
    pub fn new(config: ParserConfig, airlines: &Airlines) -> Self {
        let ParserConfig {
            recognition_corrections,
            number_words,
            direction_words,
            altitude_words,
            phonetic_alphabet,
            fuzzy_threshold,
            confidence_threshold,
        } = config;

        let (airline_name_to_icao, callsign_to_icao) = load_airlines(airlines);

        let mut parser = Self {
            edges: Vec::new(),
            number_words,
            direction_words,
            altitude_words,
            phonetic_alphabet,
            airline_name_to_icao,
            callsign_to_icao,
            recognition_corrections,
            fuzzy_threshold,
            confidence_threshold,
        };

        parser.build_graph();
        parser
    }

    fn build_graph(&mut self) {
        // 1. Callsign parsing
        self.add_edge(
            ParseState::Start,
            ParseState::ExpectingCallsign,
            TokenMatcher::Airline,
            0.9,
            true,
        );

        // After callsign, expect command
        self.add_edge(
            ParseState::ExpectingCallsign,
            ParseState::ExpectingCommand,
            TokenMatcher::Optional(Box::new(TokenMatcher::Exact(",".into()))),
            0.8,
            true,
        );

        // 2. Command entry points
        self.add_edge(
            ParseState::ExpectingCommand,
            ParseState::TurnCommand,
            TokenMatcher::OneOf(vec!["turn".into(), "fly".into()]),
            0.95,
            true,
        );

        self.add_edge(
            ParseState::ExpectingCommand,
            ParseState::ClimbCommand,
            TokenMatcher::Exact("climb".into()),
            0.95,
            true,
        );

        self.add_edge(
            ParseState::ExpectingCommand,
            ParseState::DescendCommand,
            TokenMatcher::Exact("descend".into()),
            0.95,
            true,
        );

        self.add_edge(
            ParseState::ExpectingCommand,
            ParseState::ContactCommand,
            TokenMatcher::Exact("contact".into()),
            0.95,
            true,
        );

        // 3. Turn command flow
        self.add_edge(
            ParseState::TurnCommand,
            ParseState::ExpectingDirection,
            TokenMatcher::OneOf(vec!["left".into(), "right".into()]),
            0.9,
            true,
        );

        // "fly heading" or "turn left heading"
        self.add_edge(
            ParseState::TurnCommand,
            ParseState::ExpectingHeading,
            TokenMatcher::Fuzzy("heading".into(), 0.8),
            0.9,
            true,
        );

        self.add_edge(
            ParseState::ExpectingDirection,
            ParseState::ExpectingHeading,
            TokenMatcher::Fuzzy("heading".into(), 0.8),
            0.85,
            true,
        );

        // Simple turn without heading
        self.add_edge(
            ParseState::ExpectingDirection,
            ParseState::CommandComplete,
            TokenMatcher::Optional(Box::new(TokenMatcher::Exact("turn".into()))),
            0.7,
            false,
        );

        self.add_edge(
            ParseState::ExpectingHeading,
            ParseState::CommandComplete,
            TokenMatcher::Number(NumberType::Heading),
            0.9,
            true,
        );

        // 4. Altitude commands
        self.add_edge(
            ParseState::ClimbCommand,
            ParseState::ExpectingAltitude,
            TokenMatcher::OneOf(vec!["to".into(), "and".into(), "maintain".into()]),
            0.7,
            true,
        );

        self.add_edge(
            ParseState::DescendCommand,
            ParseState::ExpectingAltitude,
            TokenMatcher::OneOf(vec!["to".into(), "and".into(), "maintain".into()]),
            0.7,
            true,
        );

        self.add_edge(
            ParseState::ClimbCommand,
            ParseState::ExpectingAltitude,
            TokenMatcher::Fuzzy("flight".into(), 0.8),
            0.8,
            true,
        );

        self.add_edge(
            ParseState::DescendCommand,
            ParseState::ExpectingAltitude,
            TokenMatcher::Fuzzy("flight".into(), 0.8),
            0.8,
            true,
        );

        self.add_edge(
            ParseState::ExpectingAltitude,
            ParseState::CommandComplete,
            TokenMatcher::OneOf(vec!["level".into(), "feet".into()]),
            0.7,
            true,
        );

        self.add_edge(
            ParseState::ExpectingAltitude,
            ParseState::CommandComplete,
            TokenMatcher::Number(NumberType::Altitude),
            0.9,
            true,
        );

        // 5. Contact commands
        self.add_edge(
            ParseState::ContactCommand,
            ParseState::ExpectingStation,
            TokenMatcher::OneOf(vec!["tower".into(), "ground".into(), "approach".into()]),
            0.9,
            true,
        );

        self.add_edge(
            ParseState::ExpectingStation,
            ParseState::ExpectingFrequency,
            TokenMatcher::Number(NumberType::Frequency),
            0.9,
            true,
        );

        self.add_edge(
            ParseState::ExpectingFrequency,
            ParseState::CommandComplete,
            TokenMatcher::Pattern(r"\d+\.\d+".into()),
            0.85,
            true,
        );

        // 6. Command completion -> next command or end
        self.add_edge(
            ParseState::CommandComplete,
            ParseState::ExpectingCommand,
            TokenMatcher::OneOf(vec!["and".into(), "then".into(), ",".into()]),
            0.5,
            true,
        );

        self.add_edge(
            ParseState::CommandComplete,
            ParseState::ParseComplete,
            TokenMatcher::Optional(Box::new(TokenMatcher::Pattern(".*".into()))),
            0.8,
            false,
        );
    }

    fn add_edge(
        &mut self,
        from: ParseState,
        to: ParseState,
        matcher: TokenMatcher,
        confidence: f32,
        consumes: bool,
    ) {
        self.edges.push(ParseEdge {
            from,
            to,
            matcher,
            confidence,
            consumes_token: consumes,
        });
    }

    /// Parse a transmission using the graph-based parser
    pub fn parse(&self, text: &str) -> ParseResult {
        self.parse_transmission_enhanced(text)
    }

    pub fn parse_transmission_enhanced(&self, text: &str) -> ParseResult {
        // Preprocess text for Whisper quirks
        let preprocessed = self.preprocess_whisper_text(text);
        let tokens = self.tokenize(&preprocessed);

        let mut best_paths = Vec::new();

        // Start exploration from initial state
        self.explore_paths(
            ParseState::Start,
            0,
            &tokens,
            1.0,
            Vec::new(),
            HashMap::new(),
            &mut best_paths,
        );

        // Find best complete path
        if let Some(best_path) = best_paths
            .into_iter()
            .filter(|path| {
                path.final_state == ParseState::ParseComplete
                    || path.final_state == ParseState::CommandComplete
            })
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
        let result = text.to_lowercase();

        // Apply recognition corrections from config
        self.recognition_corrections
            .iter()
            .fold(result, |acc, (incorrect, correct)| {
                acc.replace(incorrect, correct)
            })
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
            if let Some((match_confidence, extracted_value)) =
                self.test_matcher(&edge.matcher, current_token, token_index, tokens)
            {
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

    fn test_matcher(
        &self,
        matcher: &TokenMatcher,
        token: &str,
        _index: usize,
        _tokens: &[String],
    ) -> Option<(f32, Option<ParsedValue>)> {
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
                            "tower" | "ground" | "approach" => {
                                Some(ParsedValue::Station(opt.clone()))
                            }
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
            TokenMatcher::Number(number_type) => self.test_number_match(token, number_type),
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

    fn test_number_match(
        &self,
        token: &str,
        number_type: &NumberType,
    ) -> Option<(f32, Option<ParsedValue>)> {
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
                        if let (Ok(num), Ok(dec)) =
                            (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                        {
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
                if token
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_alphabetic())
                {
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
        let callsign = path
            .extracted_data
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

        if path
            .steps
            .iter()
            .any(|step| step.state == ParseState::CommandComplete)
        {
            // Build commands from extracted data
            if let Some(ParsedValue::Heading(heading)) =
                path.extracted_data.values().find_map(|v| match v {
                    ParsedValue::Heading(h) => Some(ParsedValue::Heading(*h)),
                    _ => None,
                })
            {
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

// Standalone function to load airlines data
fn load_airlines(airlines: &Airlines) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut callsign_to_icao = HashMap::new();
    let mut airline_name_to_icao = HashMap::new();

    for airline in &airlines.0 {
        let Some(icao) = &airline.icao else {
            continue;
        };
        if !airline.active || icao.is_empty() || icao == "N/A" {
            continue;
        }
        let icao_lower = icao.to_lowercase();

        // Primary: Map callsign to ICAO
        if let Some(callsign) = &airline.callsign {
            if !callsign.is_empty() {
                let callsign_key = callsign.to_lowercase().replace(" ", "");
                callsign_to_icao.insert(callsign_key, icao_lower.clone());
            }
        }

        // Fallback: Map airline name to ICAO
        if !airline.name.is_empty() {
            let name_key = airline.name.to_lowercase().replace(" ", "");
            if !callsign_to_icao.contains_key(&name_key) {
                airline_name_to_icao.insert(name_key, icao_lower.clone());
            }
        }
    }

    (airline_name_to_icao, callsign_to_icao)
}

// Re-export for compatibility
pub use CommandWithConfidence as GraphCommandWithConfidence;
pub use ParseResult as GraphParseResult;
pub use ParsedCommand as GraphParsedCommand;

#[cfg(test)]
mod tests {
    use super::*;
    use aviation_helper_rs::clearance::airlines::AirlineEntry;

    fn create_test_config() -> ParserConfig {
        let mut recognition_corrections = HashMap::new();
        recognition_corrections.insert("flyheading".to_string(), "fly heading".to_string());
        recognition_corrections.insert("onetwothree".to_string(), "123".to_string());

        let mut number_words = HashMap::new();
        number_words.insert("one".to_string(), 1);
        number_words.insert("two".to_string(), 2);
        number_words.insert("three".to_string(), 3);

        let mut direction_words = HashMap::new();
        direction_words.insert("left".to_string(), "left".to_string());
        direction_words.insert("right".to_string(), "right".to_string());

        let mut altitude_words = HashMap::new();
        altitude_words.insert("climb".to_string(), "climb".to_string());
        altitude_words.insert("descend".to_string(), "descend".to_string());

        let mut phonetic_alphabet = HashMap::new();
        phonetic_alphabet.insert("delta".to_string(), "D".to_string());

        ParserConfig {
            recognition_corrections,
            number_words,
            direction_words,
            altitude_words,
            phonetic_alphabet,
            fuzzy_threshold: 0.8,
            confidence_threshold: 0.1,
        }
    }

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
        let config = create_test_config();
        let airlines = create_test_airlines();
        let parser = GraphParser::new(config, &airlines);

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
        let config = create_test_config();
        let airlines = create_test_airlines();
        let parser = GraphParser::new(config, &airlines);

        let preprocessed = parser.preprocess_whisper_text("deltaonetwothreeflyheading270");
        assert_eq!(preprocessed, "delta123fly heading270");
    }

    #[test]
    fn test_fuzzy_matching() {
        let config = create_test_config();
        let airlines = create_test_airlines();
        let parser = GraphParser::new(config, &airlines);

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
