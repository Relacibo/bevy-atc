use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use serde::Deserialize;

use crate::game::TurnDirection;

use super::heading::Heading;

#[derive(Clone, Debug, Component)]
pub struct Aircraft {
    pub aircraft_type_id: String,
    pub call_sign: String,
    pub cleared_altitude_feet: Option<f64>,
    pub wanted_altitude_feet: f64,
    pub cleared_heading: Option<Heading>,
    pub cleared_heading_change_direction: Option<TurnDirection>,
    pub cleared_speed_knots: Option<f64>,
    pub wanted_speed_knots: f64,
    pub altitude_feet: f64,
    pub altitude_change_feet_per_second: f64,
    pub heading: Heading,
    pub heading_change_degrees_per_second: f64,
    pub speed_knots: f64,
    pub acceleration_knots_per_second: f64,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftTypeMeta {
    pub id: String,
    pub file: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftTypeIndexFile {
    pub types: Vec<AircraftTypeMeta>,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub characteristics: Vec<AircraftCharacteristic>,
    pub heading_accuracy_degrees: f64,
    pub max_delta_heading_degrees_per_second: f64,
    pub delta_heading_acceleration_degrees_per_second: f64,
    pub speed_accuracy_knots: f64,
    pub max_delta_speed_knots_per_second: f64,
    pub delta_speed_acceleration_knots_per_second: f64,
    pub altitude_accuracy_feet: f64,
    pub max_delta_altitude_feet_per_second: f64,
    pub delta_altitude_acceleration_feet_per_second: f64,
    pub optimal_cruising_altitude_feet: f64,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub enum AircraftCharacteristic {
    Heavy,
}
