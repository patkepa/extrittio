use std::process::Command;

#[test]
fn preflight_rejects_missing_and_invalid_contracts_without_starting_client() {
    let directory = tempfile::tempdir().unwrap();
    let contract = directory.path().join("device-contract.json");
    let missing = Command::new(env!("CARGO_BIN_EXE_extrittio-rpi"))
        .arg("--contract")
        .arg(&contract)
        .arg("--check-contract")
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("Contract validation failed"));

    std::fs::write(&contract, b"{}").unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_extrittio-rpi"))
        .arg("--contract")
        .arg(&contract)
        .arg("--check-contract")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("has no document"));
}
