pub mod conversions {
    pub const MILES_PER_SECONDS_TO_KNOTS: f64 = 3128.30820878;
    pub const KNOTS_TO_MILES_PER_SECOND: f64 = 0.000319662;
    pub const RADIANS_TO_DEGREES: f64 = 90. / std::f64::consts::FRAC_2_PI;
    pub const DEGREES_TO_RADIANS: f64 = std::f64::consts::FRAC_PI_4 / 45.;
}

pub const PIXELS_PER_MILE: u32 = 10;
pub const FIXED_UPDATES_PER_SECOND: u32 = 60;
pub const FIXED_UPDATE_LENGTH_SECOND: f32 = 1. / FIXED_UPDATES_PER_SECOND as f32;
pub const PIXEL_PER_KNOT_SECOND: f64 =
    PIXELS_PER_MILE as f64 * conversions::KNOTS_TO_MILES_PER_SECOND;
