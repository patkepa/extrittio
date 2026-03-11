/// Simulated sensor readings with random-walk behavior.
pub struct SensorState {
    pub temperature: f32,
    pub humidity: f32,
    pub battery: f32,
}

impl SensorState {
    pub fn new() -> Self {
        Self {
            temperature: 22.0,
            humidity: 45.0,
            battery: 100.0,
        }
    }

    /// Advance the sensor state by one step.
    ///
    /// The caller provides random offsets so the SDK does not depend on any
    /// specific `rand` version:
    /// - `temp_offset`: value in `[-0.5, 0.5]`
    /// - `humidity_offset`: value in `[-1.0, 1.0]`
    /// - `battery_drain`: value in `[0.05, 0.15]`
    pub fn step(&mut self, temp_offset: f32, humidity_offset: f32, battery_drain: f32) {
        self.temperature = (self.temperature + temp_offset).clamp(15.0, 30.0);
        self.humidity = (self.humidity + humidity_offset).clamp(20.0, 80.0);
        self.battery = (self.battery - battery_drain).max(0.0);
    }
}

impl Default for SensorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_clamps_temperature() {
        let mut s = SensorState { temperature: 30.0, humidity: 50.0, battery: 50.0 };
        s.step(1.0, 0.0, 0.0);
        assert_eq!(s.temperature, 30.0);

        s.temperature = 15.0;
        s.step(-1.0, 0.0, 0.0);
        assert_eq!(s.temperature, 15.0);
    }

    #[test]
    fn test_step_drains_battery() {
        let mut s = SensorState::new();
        s.step(0.0, 0.0, 0.1);
        assert!((s.battery - 99.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_battery_does_not_go_negative() {
        let mut s = SensorState { temperature: 22.0, humidity: 45.0, battery: 0.01 };
        s.step(0.0, 0.0, 0.1);
        assert_eq!(s.battery, 0.0);
    }
}
