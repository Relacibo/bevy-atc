pub const MILES_PER_SECONDS_TO_KNOTS: f64 = 3128.30820878;
pub const KNOTS_TO_MILES_PER_SECOND: f64 = 0.000319662;
pub const RADIANS_TO_DEGREES: f64 = 90. / std::f64::consts::FRAC_2_PI;
pub const DEGREES_TO_RADIANS: f64 = std::f64::consts::FRAC_PI_4 / 45.;

pub fn degrees_to_rotation(degrees: f64) -> f64 {
    ((degrees + 90.) % 360. - 180.) * DEGREES_TO_RADIANS
}

pub fn rotation_to_degrees(rotation: f64) -> f64 {
    (rotation * RADIANS_TO_DEGREES + 90.) % 360.
}
