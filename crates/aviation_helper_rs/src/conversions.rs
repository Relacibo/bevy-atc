pub const MILES_PER_SECONDS_TO_KNOTS: f64 = 3128.30820878;
pub const KNOTS_TO_MILES_PER_SECOND: f64 = 0.000319662;
pub const RADIANS_TO_DEGREES: f64 = 180.0 / std::f64::consts::PI;
pub const DEGREES_TO_RADIANS: f64 = std::f64::consts::PI / 180.0;

pub fn aviation_degrees_to_bevy_rotation(degrees: f64) -> f64 {
    (90.0 - degrees).to_radians()
}

pub fn bevy_rotation_to_aviation_degrees(rotation: f64) -> f64 {
    let degrees = 90.0 - rotation.to_degrees();
    degrees.rem_euclid(360.0)
}
