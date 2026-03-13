#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
pub mod extrittio {
    include!(concat!(env!("OUT_DIR"), "/extrittio.rs"));
}

pub mod device_status;
pub mod ota;
#[cfg(feature = "std")]
pub mod shadow;
pub mod topics;

#[cfg(all(test, feature = "std"))]
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
            correlation_id: "abc-123".to_string(),
        };

        let bytes = command.encode_to_vec();
        let decoded = DeviceCommand::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded.command, "restart");
        assert_eq!(decoded.params.get("delay").unwrap(), "5");
        assert_eq!(decoded.correlation_id, "abc-123");
    }

    #[test]
    fn test_command_response_roundtrip() {
        let response = DeviceCommandResponse {
            correlation_id: "abc-123".to_string(),
            device_id: "dev-001".to_string(),
            status: "succeeded".to_string(),
            payload: r#"{"result":"ok"}"#.to_string(),
            timestamp: 1709900000000,
        };

        let bytes = response.encode_to_vec();
        let decoded = DeviceCommandResponse::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded.correlation_id, "abc-123");
        assert_eq!(decoded.device_id, "dev-001");
        assert_eq!(decoded.status, "succeeded");
        assert_eq!(decoded.payload, r#"{"result":"ok"}"#);
    }
}
