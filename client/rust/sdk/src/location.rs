/// Simulated GPS location state for testing without hardware.
pub struct LocationState {
    pub latitude: f64,
    pub longitude: f64,
    pub speed: f32,
    pub altitude: f32,
    pub heading: f32,
}

impl LocationState {
    pub fn new(center_lat: f64, center_lon: f64) -> Self {
        Self {
            latitude: center_lat,
            longitude: center_lon,
            speed: 0.0,
            altitude: 100.0,
            heading: 0.0,
        }
    }

    pub fn step(&mut self, lat_offset: f64, lon_offset: f64, speed_offset: f32) {
        self.latitude += lat_offset;
        self.longitude += lon_offset;
        if lat_offset.abs() > f64::EPSILON || lon_offset.abs() > f64::EPSILON {
            self.heading = (lon_offset.atan2(lat_offset).to_degrees() as f32 + 360.0) % 360.0;
        }
        self.speed = (self.speed + speed_offset).clamp(0.0, 50.0);
    }
}

impl Default for LocationState {
    fn default() -> Self {
        Self::new(52.2297, 21.0122)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let loc = LocationState::new(40.7128, -74.0060);
        assert_eq!(loc.latitude, 40.7128);
        assert_eq!(loc.longitude, -74.0060);
    }

    #[test]
    fn test_step_moves_position() {
        let mut loc = LocationState::new(52.0, 21.0);
        loc.step(0.001, 0.002, 5.0);
        assert!((loc.latitude - 52.001).abs() < f64::EPSILON);
        assert!((loc.longitude - 21.002).abs() < f64::EPSILON);
    }

    #[test]
    fn test_speed_clamped() {
        let mut loc = LocationState::new(52.0, 21.0);
        loc.step(0.0, 0.0, -100.0);
        assert_eq!(loc.speed, 0.0);
    }
}
