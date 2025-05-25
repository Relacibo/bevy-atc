use super::consts;

pub fn degrees_to_rotation(degrees: f64) -> f64 {
    ((degrees + 90.) % 360. - 180.) * consts::conversions::DEGREES_TO_RADIANS
}

pub fn rotation_to_degrees(rotation: f64) -> f64 {
    (rotation * consts::conversions::RADIANS_TO_DEGREES + 90.) % 360.
}
