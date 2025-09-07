use std::{ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    errors::Error,
    types::{
        altitude::{Altitude, VerticalDirection},
        heading::{CardinalDirection, Degrees, Heading, TurnDirection},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum HeadingDirection {
    RunwayHeading,
    CardinalDirection(CardinalDirection),
    Heading(Heading),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Frequency {
    pub num: u32,
    pub dec: u32,
}

impl FromStr for Frequency {
    type Err = Error;
    fn from_str(val: &str) -> Result<Self, Self::Err> {
        let mut split = val.split(".");
        match (split.next(), split.next(), split.next()) {
            (Some(num), dec, None) => {
                let num: u32 = num
                    .parse()
                    .map_err(|_| Error::InvalidFrequency(val.to_owned()))?;
                let dec: u32 = dec
                    .map(|d| d.parse())
                    .transpose()
                    .map_err(|_| Error::InvalidFrequency(val.to_owned()))?
                    .unwrap_or_default();
                Ok(Self { num, dec })
            }
            _ => Err(Error::InvalidFrequency(val.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum AviationCommandPart {
    RadarContact,
    TurnBy {
        degrees: Degrees,
        turn_direction: Option<TurnDirection>,
    },
    FlyHeading {
        heading: HeadingDirection,
        turn_direction: Option<TurnDirection>,
    },
    ProceedDirect(String),
    ChangeAltitude {
        altitude: Altitude,
        #[serde(default)]
        maintain: bool,
        turn_direction: Option<VerticalDirection>,
    },
    ContactFrequency {
        frequency: Frequency,
        station: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum CommunicationEntity {
    All,
    GroundStation { full_name: String },
    Aircraft { full_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AviationCommandGroup {
    pub target: Option<CommunicationEntity>,
    pub parts: Vec<AviationCommandPart>,
}
