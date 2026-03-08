pub mod extrittio {
    include!(concat!(env!("OUT_DIR"), "/extrittio.rs"));
}

#[cfg(test)]
mod tests {
    use super::extrittio::*;
    use prost::Message;

    #[test]
    fn test_telemetry_roundtrip() {
        let telemetry = DeviceTelemetry {
            device_id: "dev-001".to_string(),
            timestamp: 1709900000000,
            temperature: 22.5,
            humidity: 45.0,
            battery_level: 87.3,
            metadata: [("location".to_string(), "room-a".to_string())].into(),
        };

        let bytes = telemetry.encode_to_vec();
        let decoded = DeviceTelemetry::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded.device_id, "dev-001");
        assert!((decoded.temperature - 22.5).abs() < f32::EPSILON);
        assert_eq!(decoded.metadata.get("location").unwrap(), "room-a");
    }

    #[test]
    fn test_heartbeat_roundtrip() {
        let heartbeat = DeviceHeartbeat {
            device_id: "dev-001".to_string(),
            timestamp: 1709900000000,
            status: "online".to_string(),
            firmware: "v2.1.0".to_string(),
            uptime_seconds: 86400,
        };

        let bytes = heartbeat.encode_to_vec();
        let decoded = DeviceHeartbeat::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded.device_id, "dev-001");
        assert_eq!(decoded.status, "online");
        assert_eq!(decoded.uptime_seconds, 86400);
    }

    #[test]
    fn test_command_roundtrip() {
        let command = DeviceCommand {
            command: "restart".to_string(),
            params: [("delay".to_string(), "5".to_string())].into(),
        };

        let bytes = command.encode_to_vec();
        let decoded = DeviceCommand::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded.command, "restart");
        assert_eq!(decoded.params.get("delay").unwrap(), "5");
    }
}
