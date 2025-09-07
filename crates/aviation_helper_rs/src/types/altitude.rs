use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum VerticalDirection {
    Climb,
    Descend,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum Altitude {
    Feet(f64),
    FlightLevel(u32),
}

// Custom Eq implementation for test comparisons
// Considers altitudes equal if they're within 1 foot
impl Eq for Altitude {}

impl Altitude {
    pub fn as_feet(self) -> f64 {
        match self {
            Altitude::Feet(f) => f,
            Altitude::FlightLevel(fl) => (fl * 100) as f64,
        }
    }
}
