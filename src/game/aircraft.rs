use bevy::prelude::*;

use super::heading::Heading;

#[derive(Clone, Debug, Component)]
pub struct Aircraft {
    pub call_sign: String,
    pub cleared_altitude_feet: Option<f64>,
    pub cleared_heading: Option<Heading>,
    pub cleared_speed_knots: Option<f64>,
    pub wanted_speed_knots: f64,
}

#[derive(Clone, Debug, Component)]
pub struct AircraftPhysics {
    pub altitude_feet: f64,
    pub altitude_change_feet_per_second: f64,
    pub heading: Heading,
    pub heading_change_degrees_per_second: f64,
    pub speed_knots: f64,
    pub acceleration_knots_per_second: f64,
}
