use bevy::prelude::*;

use super::heading::Heading;

#[derive(Clone, Debug, Component)]
pub struct Aircraft {
    pub call_sign: String,
    pub cleared_altitude_feet: Option<i32>,
    pub cleared_heading: Option<Heading>,
    pub cleared_speed_knots: Option<f32>,
    pub wanted_speed_knots: f32,
}

#[derive(Clone, Debug, Component)]
pub struct AircraftPhysics {
    pub altitude_feet: f32,
    pub altitude_change: f32,
    pub heading: Heading,
    pub heading_change: f64,
    pub speed_knots: f32,
    pub acceleration_knots_per_second: f32,
}
