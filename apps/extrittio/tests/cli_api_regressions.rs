use std::{
    fs,
    io::{Read as _, Write as _},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn extrittio(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_extrittio"))
        .args(args)
        .output()
        .expect("the extrittio test binary should start")
}

fn temp_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("extrittio-{name}-{}-{nonce}", std::process::id()))
}

fn serve_once(response_body: &'static str) -> (String, mpsc::Receiver<String>) {
    serve_responses(vec![response_body.to_owned()])
}

fn serve_responses(responses: Vec<String>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for response_body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let header_end = loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "client closed before sending HTTP headers");
                request.extend_from_slice(&buffer[..read]);
                if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    break position + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while request.len() - header_end < content_length {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "client closed before sending the HTTP body");
                request.extend_from_slice(&buffer[..read]);
            }
            sender
                .send(String::from_utf8_lossy(&request).into_owned())
                .unwrap();

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });

    (format!("http://{address}"), receiver)
}

#[test]
fn api_command_reports_connection_errors_instead_of_panicking() {
    let config = temp_path("missing-config");
    let output = extrittio(&[
        "--url",
        "http://127.0.0.1:9",
        "--config",
        config.to_str().unwrap(),
        "health",
    ]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("request failed"),
        "unexpected stderr: {stderr}"
    );
    assert!(!stderr.to_ascii_lowercase().contains("panicked"));
}

#[test]
fn device_create_requires_a_non_blank_blueprint_revision() {
    for args in [
        vec!["devices", "create", "--name", "sensor"],
        vec![
            "devices",
            "create",
            "--name",
            "sensor",
            "--blueprint-revision-id",
            "   ",
        ],
    ] {
        let output = extrittio(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("blueprint-revision-id"),
            "unexpected stderr: {stderr}"
        );
    }
}

#[test]
fn device_create_sends_the_blueprint_revision() {
    const RESPONSE: &str = r#"{"id":"device-1","name":"sensor","blueprint_id":"blueprint-1","blueprint_revision_id":"revision-123","blueprint_key":"sensor","blueprint_name":"Sensor","blueprint_icon":null,"blueprint_color":null,"fleet_id":null,"fleet_name":null,"status":"offline","last_seen":"never","last_seen_at":null,"firmware":"unknown","uptime":"0s","uptime_seconds":0,"declared_connections":[]}"#;
    let (url, request) = serve_once(RESPONSE);
    let config = temp_path("device-config");
    let output = extrittio(&[
        "--url",
        &url,
        "--token",
        "test-token",
        "--config",
        config.to_str().unwrap(),
        "--output",
        "json",
        "devices",
        "create",
        "--name",
        "sensor",
        "--blueprint-revision-id",
        "revision-123",
    ]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = request.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("POST /api/v1/devices HTTP/1.1"));
    assert!(request.contains(r#""blueprint_revision_id":"revision-123""#));
    assert!(!request.contains("device_type"));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["blueprint_id"], "blueprint-1");
    assert_eq!(json["blueprint_revision_id"], "revision-123");
    assert_eq!(json["blueprint_key"], "sensor");
    assert_eq!(json["declared_connections"], serde_json::json!([]));
    for field in [
        "device_type_id",
        "device_type_name",
        "latest_latitude",
        "latest_longitude",
    ] {
        assert!(json.get(field).is_none(), "{field}");
    }
}

#[test]
fn retired_device_type_commands_and_creation_selectors_are_rejected() {
    for args in [
        vec!["device-types", "list"],
        vec![
            "devices",
            "create",
            "--name",
            "sensor",
            "--blueprint-revision-id",
            "revision",
            "--device-type",
            "sensor",
        ],
        vec![
            "devices",
            "create",
            "--name",
            "sensor",
            "--blueprint-revision-id",
            "revision",
            "--device-type-id",
            "1",
        ],
    ] {
        let output = extrittio(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("device-type"));
    }
}

#[test]
fn device_tables_display_blueprint_identity() {
    let device = serde_json::json!({
        "id":"device", "name":"Example", "blueprint_id":"bp-opaque",
        "blueprint_revision_id":"rev-opaque", "blueprint_key":"arbitrary",
        "blueprint_name":"Arbitrary", "status":"offline", "last_seen":"never",
        "firmware":"1", "uptime":"0s", "uptime_seconds":0,
        "declared_connections":[]
    });
    let page = serde_json::json!({"data":[device.clone()],"total":1,"limit":10,"offset":0});
    let (url, requests) = serve_responses(vec![device.to_string(), page.to_string()]);
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.json");
    for (args, expected) in [
        (
            vec!["devices", "get", "device"],
            vec![
                "blueprint=Arbitrary (bp-opaque)",
                "blueprint_revision=rev-opaque",
            ],
        ),
        (vec!["devices", "list"], vec!["BLUEPRINT", "Arbitrary"]),
    ] {
        let mut command = vec![
            "--url",
            url.as_str(),
            "--token",
            "test-token",
            "--config",
            config.to_str().unwrap(),
        ];
        command.extend(args);
        let output = extrittio(&command);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        for value in expected {
            assert!(text.contains(value), "{text}");
        }
        assert!(!text.contains("device_type"));
        assert!(
            requests
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .starts_with("GET /api/v1/devices")
        );
    }
}

#[test]
fn firmware_upload_requires_and_sends_the_blueprint_revision() {
    let missing = extrittio(&["firmware", "upload", "--file", "/does/not/matter.bin"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("blueprint-revision-id")
    );

    const RESPONSE: &str = r#"{"id":7,"blueprint_revision_id":"revision-456","compatibility":{},"update_strategy":null,"version":"1.2.3","url":"/firmware/7/download","sha256":null,"description":null,"created_at":"2026-09-02T00:00:00Z","has_blob":true,"file_size":4,"filename":"firmware.bin","commit_sha":null,"branch":null,"ci_run_url":null,"build_timestamp":null,"changelog":null,"source":"upload"}"#;
    let (url, request) = serve_once(RESPONSE);
    let firmware = temp_path("firmware.bin");
    fs::write(&firmware, b"test").unwrap();
    let config = temp_path("firmware-config");
    let output = extrittio(&[
        "--url",
        &url,
        "--token",
        "test-token",
        "--config",
        config.to_str().unwrap(),
        "firmware",
        "upload",
        "--blueprint-revision-id",
        "revision-456",
        "--file",
        firmware.to_str().unwrap(),
    ]);
    fs::remove_file(&firmware).unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = request.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("POST /api/v1/firmware-updates/upload HTTP/1.1"));
    assert!(request.contains("name=\"blueprint_revision_id\""));
    assert!(request.contains("revision-456"));
    assert!(!request.contains("device_type"));
}

#[test]
fn firmware_list_encodes_revision_filter_and_rejects_device_type_selectors() {
    let (url, request) = serve_once(r#"{"data":[],"total":0,"limit":50,"offset":0}"#);
    let config = temp_path("firmware-list-config");
    let result = extrittio(&[
        "--url",
        &url,
        "--token",
        "test-token",
        "--config",
        config.to_str().unwrap(),
        "firmware",
        "list",
        "--blueprint-revision-id",
        "revision/a&b",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(request.recv_timeout(Duration::from_secs(5)).unwrap().starts_with(
        "GET /api/v1/firmware-updates?limit=50&offset=0&blueprint_revision_id=revision%2Fa%26b HTTP/1.1"
    ));
    for args in [
        vec!["firmware", "list", "--device-type-id", "1"],
        vec![
            "firmware",
            "upload",
            "--file",
            "firmware.bin",
            "--blueprint-revision-id",
            "revision",
            "--device-type-id",
            "1",
        ],
    ] {
        let result = extrittio(&args);
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("device-type-id"));
    }
}
