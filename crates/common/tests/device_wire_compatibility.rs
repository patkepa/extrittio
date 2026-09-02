use std::{collections::HashMap, fmt::Debug};

use extrittio_common::extrittio::{
    DeviceCommand, DeviceCommandResponse, DeviceHeartbeat, DeviceLog, DeviceTelemetry, ShadowDelta,
    ShadowGet, ShadowReport,
};
use prost::Message;

fn fixture_bytes(fixture: &str) -> Vec<u8> {
    let hex = fixture
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(hex.len() % 2, 0, "wire fixture must contain whole bytes");

    hex.chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("fixture must be ASCII hex");
            u8::from_str_radix(pair, 16).expect("fixture must contain only hexadecimal digits")
        })
        .collect()
}

fn assert_wire_fixture<M>(fixture: &str, expected: M)
where
    M: Message + Default + PartialEq + Debug,
{
    let fixture = fixture_bytes(fixture);
    let decoded = M::decode(fixture.as_slice()).expect("historical fixture must remain decodable");

    assert_eq!(decoded, expected, "fixture semantics changed");
    assert_eq!(
        expected.encode_to_vec(),
        fixture,
        "current encoder changed the established wire bytes"
    );
}

#[test]
fn device_telemetry_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/device-telemetry-v1.hex"),
        DeviceTelemetry {
            device_id: "dev-1".into(),
            timestamp: 42,
            temperature: 1.5,
            humidity: 2.5,
            battery_level: 3.5,
            metadata: HashMap::from([("site".into(), "lab".into())]),
            latitude: 4.5,
            longitude: 5.5,
            speed: 6.5,
            altitude: 7.5,
            heading: 8.5,
            has_location: false,
        },
    );
}

#[test]
fn device_heartbeat_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/device-heartbeat-v1.hex"),
        DeviceHeartbeat {
            device_id: "dev-1".into(),
            timestamp: 42,
            status: "online".into(),
            firmware: "1.2.3".into(),
            uptime_seconds: 300,
        },
    );
}

#[test]
fn device_command_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/device-command-v1.hex"),
        DeviceCommand {
            command: "reboot".into(),
            params: HashMap::from([("delay".into(), "5".into())]),
            correlation_id: "corr-1".into(),
        },
    );
}

#[test]
fn device_command_response_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/device-command-response-v1.hex"),
        DeviceCommandResponse {
            correlation_id: "corr-1".into(),
            device_id: "dev-1".into(),
            status: "succeeded".into(),
            payload: r#"{"ok":true}"#.into(),
            timestamp: 42,
        },
    );
}

#[test]
fn shadow_report_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/shadow-report-v1.hex"),
        ShadowReport {
            device_id: "dev-1".into(),
            timestamp: 42,
            state_json: r#"{"temp":21}"#.into(),
            version: 7,
        },
    );
}

#[test]
fn shadow_delta_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/shadow-delta-v1.hex"),
        ShadowDelta {
            device_id: "dev-1".into(),
            delta_json: r#"{"led":true}"#.into(),
            version: 8,
        },
    );
}

#[test]
fn shadow_get_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/shadow-get-v1.hex"),
        ShadowGet {
            device_id: "dev-1".into(),
        },
    );
}

#[test]
fn device_log_v1_wire_bytes_remain_compatible() {
    assert_wire_fixture(
        include_str!("fixtures/device-log-v1.hex"),
        DeviceLog {
            device_id: "dev-1".into(),
            timestamp: 42,
            level: "warn".into(),
            message: "battery low".into(),
        },
    );
}
