use std::process::{Command, Output};

const ROOT_HELP: &str = include_str!("fixtures/root-help.txt");
const INVALID_COMMAND: &str = include_str!("fixtures/invalid-command.stderr");
const MISSING_DEVICE_ID: &str = include_str!("fixtures/missing-device-id.stderr");

fn extrittio(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_extrittio"))
        .args(args)
        .output()
        .expect("the extrittio test binary should start")
}

#[test]
fn root_help_is_stable_and_exits_successfully() {
    let output = extrittio(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), ROOT_HELP);
    assert!(output.stderr.is_empty());
}

#[test]
fn version_is_written_to_stdout_and_exits_successfully() {
    let output = extrittio(&["--version"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!("extrittio ", env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_command_is_written_to_stderr_and_exits_with_two() {
    let output = extrittio(&["definitely-not-a-command"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), INVALID_COMMAND);
}

#[test]
fn missing_required_argument_is_written_to_stderr_and_exits_with_two() {
    let output = extrittio(&["devices", "get"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), MISSING_DEVICE_ID);
}

#[test]
fn server_alias_and_global_options_after_a_subcommand_keep_parsing() {
    let serve = extrittio(&["serve", "--help"]);
    let server_alias = extrittio(&["server", "--help"]);
    assert_eq!(serve.status.code(), Some(0));
    assert_eq!(server_alias.status.code(), Some(0));
    assert_eq!(server_alias.stdout, serve.stdout);
    assert!(server_alias.stderr.is_empty());

    let plain = extrittio(&["devices", "list", "--help"]);
    let with_globals = extrittio(&[
        "devices",
        "list",
        "--output",
        "json",
        "--url",
        "https://hub.example.test",
        "--token",
        "fixture-token",
        "--config",
        "/tmp/extrittio-fixture.toml",
        "--help",
    ]);
    assert_eq!(plain.status.code(), Some(0));
    assert_eq!(with_globals.status.code(), Some(0));
    assert_eq!(with_globals.stdout, plain.stdout);
    assert!(with_globals.stderr.is_empty());
}
