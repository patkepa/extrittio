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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
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
            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
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
        vec![
            "devices",
            "create",
            "--name",
            "sensor",
            "--device-type-id",
            "1",
        ],
        vec![
            "devices",
            "create",
            "--name",
            "sensor",
            "--device-type-id",
            "1",
            "--blueprint-revision-id",
            "   ",
        ],
        vec!["provision", "--name", "sensor", "--device-type-id", "1"],
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
    const RESPONSE: &str = r#"{"id":"device-1","name":"sensor","device_type_id":1,"device_type_name":"Sensor","fleet_id":null,"fleet_name":null,"status":"offline","last_seen":"never","last_seen_at":null,"firmware":"unknown","uptime":"0s","uptime_seconds":0,"latest_latitude":null,"latest_longitude":null}"#;
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
        "--device-type-id",
        "1",
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
}

#[test]
fn firmware_upload_requires_and_sends_the_blueprint_revision() {
    let missing = extrittio(&[
        "firmware",
        "upload",
        "--device-type-id",
        "1",
        "--file",
        "/does/not/matter.bin",
    ]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("blueprint-revision-id")
    );

    const RESPONSE: &str = r#"{"id":7,"device_type_id":1,"device_type_name":"Sensor","version":"1.2.3","url":"/firmware/7/download","sha256":null,"description":null,"created_at":"2026-09-02T00:00:00Z","has_blob":true,"file_size":4,"filename":"firmware.bin","commit_sha":null,"branch":null,"ci_run_url":null,"build_timestamp":null,"changelog":null,"source":"upload"}"#;
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
        "--device-type-id",
        "1",
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
}
