use std::ops::{Add, Sub};

use crate::util::conversions::{degrees_to_rotation, rotation_to_degrees};

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Heading(f64);

impl From<f64> for Heading {
    fn from(value: f64) -> Self {
        Heading(value.rem_euclid(360.))
    }
}

impl Add for Heading {
    type Output = f64;

    fn add(self, rhs: Self) -> Self::Output {
        (self.0 + rhs.0) % 360.
    }
}

impl Sub for Heading {
    type Output = f64;

    fn sub(self, rhs: Self) -> Self::Output {
        (self.0 - rhs.0).rem_euclid(360.)
    }
}

impl Heading {
    pub fn change(self, heading_change: f64) -> Heading {
        let Heading(heading) = self;
        let res = (heading + heading_change).rem_euclid(360.);
        Heading(res)
    }

    pub fn to_rotation(self) -> f64 {
        let Heading(heading) = self;
        degrees_to_rotation(heading)
    }

    pub fn from_rotation(value: f64) -> Self {
        let heading = rotation_to_degrees(value);
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
