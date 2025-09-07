use std::{
    fmt::Display,
    ops::{Add, Deref, Sub},
};

use serde::{Deserialize, Serialize};

use crate::conversions::{aviation_degrees_to_bevy_rotation, bevy_rotation_to_aviation_degrees};

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Heading(f64);

// Custom Eq implementation for test comparisons
// Considers headings equal if they're within 0.1 degrees
impl Eq for Heading {}

impl PartialEq<f64> for Heading {
    fn eq(&self, other: &f64) -> bool {
        (self.0 - other).abs() < 0.1
    }
}

impl From<f64> for Heading {
    fn from(value: f64) -> Self {
        Heading(value.rem_euclid(360.))
    }
}

impl Add for Heading {
    type Output = f64;

    fn add(self, rhs: Self) -> Self::Output {
        (self.0 + rhs.0).rem_euclid(360.)
    }
}

impl Sub for Heading {
    type Output = f64;

    fn sub(self, rhs: Self) -> Self::Output {
        (self.0 - rhs.0).rem_euclid(360.)
    }
}

impl Add<f64> for Heading {
    type Output = Heading;

    fn add(self, rhs: f64) -> Self::Output {
        Heading((self.0 + rhs).rem_euclid(360.))
    }
}

impl Sub<f64> for Heading {
    type Output = Heading;

    fn sub(self, rhs: f64) -> Self::Output {
        Heading((self.0 - rhs).rem_euclid(360.))
    }
}

impl Heading {
    pub fn new(val: f64) -> Self {
        Heading(val.rem_euclid(360.))
    }

    pub fn to_bevy_rotation(self) -> f64 {
        let Heading(heading) = self;
        aviation_degrees_to_bevy_rotation(heading)
    }

    pub fn from_bevy_rotation(value: f64) -> Self {
        let heading = bevy_rotation_to_aviation_degrees(value);
        Heading(heading)
    }

    pub fn required_change(self, cleared: Heading) -> f64 {
        let distance = self - cleared;
        if distance < 180.0 {
            -distance
        } else {
            360. - distance
        }
    }

    pub fn get(&self) -> f64 {
        self.0
    }
}

impl Display for Heading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = match self.0.floor() {
            0.0 => 360,
            n => n as i32,
        };
        write!(f, "{num:03}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum CardinalDirection {
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deserialize, Serialize)]
pub enum TurnDirection {
    Stay,
    Left,
    Right,
}

/// Wrapper for degrees that implements Eq for test comparisons  
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Degrees(pub f64);

impl Eq for Degrees {}

impl From<f64> for Degrees {
    fn from(value: f64) -> Self {
        Degrees(value)
    }
}

impl Deref for Degrees {
    type Target = f64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
