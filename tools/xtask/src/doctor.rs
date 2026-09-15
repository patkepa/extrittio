use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;

use crate::command::{capture, command_in};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Scope {
    All,
    Edge,
    Cloud,
    Ios,
}

pub(crate) fn run(root: &Path, scope: Scope) -> Result<()> {
    let expectations = Expectations::read(root)?;
    let pg_config = if Path::new("/opt/homebrew/opt/libpq/bin/pg_config").is_file() {
        "/opt/homebrew/opt/libpq/bin/pg_config"
    } else if Path::new("/usr/local/opt/libpq/bin/pg_config").is_file() {
        "/usr/local/opt/libpq/bin/pg_config"
    } else {
        "pg_config"
    };
    let mut checks = Vec::new();
    if matches!(scope, Scope::All | Scope::Edge | Scope::Cloud) {
        checks.extend([
        Check::required(
            "Rust toolchain",
            "rustc",
            &["--version"],
            Expected::Word(expectations.rust.clone()),
            format!(
                "run `rustup toolchain install {} --component rustfmt,clippy`",
                expectations.rust
            ),
        ),
        Check::required(
            "Cargo",
            "cargo",
            &["--version"],
            Expected::Any,
            "install Rust through rustup: https://rustup.rs".into(),
        ),
        Check::required(
            "Node.js",
            "node",
            &["--version"],
            Expected::MajorAtLeast(expectations.node_major),
            format!(
                "install Node.js {} or newer (for example with nvm)",
                expectations.node_major
            ),
        ),
        Check::required(
            "npm",
            "npm",
            &["--version"],
            Expected::Exact(expectations.npm.clone()),
            format!(
                "run `corepack prepare npm@{} --activate` or install the packageManager version from apps/frontend/package.json",
                expectations.npm
            ),
        ),
        Check::required(
            "Protocol Buffers compiler",
            "protoc",
            &["--version"],
            Expected::Any,
            "install `protobuf` with Homebrew or `protobuf-compiler` with apt".into(),
        ),
        ]);
    }
    if matches!(scope, Scope::All) {
        checks.extend([
            Check::required(
                "rustfmt",
                "rustfmt",
                &["--version"],
                Expected::Any,
                "run `rustup component add rustfmt`".into(),
            ),
            Check::required(
                "Clippy",
                "cargo",
                &["clippy", "--version"],
                Expected::Any,
                "run `rustup component add clippy`".into(),
            ),
        ]);
    }
    if matches!(scope, Scope::All | Scope::Cloud) {
        checks.extend([
        Check::required(
            "Docker CLI",
            "docker",
            &["--version"],
            Expected::Any,
            "install and start Docker Desktop, or install Docker Engine".into(),
        ),
        Check::required(
            "PostgreSQL client libraries",
            pg_config,
            &["--version"],
            Expected::Any,
            "install `libpq` with Homebrew or `libpq-dev` with apt and add its bin directory to PATH".into(),
        ),
        ]);
    }
    if matches!(scope, Scope::All) {
        checks.extend([
        Check::required(
            "CMake",
            "cmake",
            &["--version"],
            Expected::Any,
            "install `cmake` with Homebrew or apt".into(),
        ),
        Check::required(
            "Ninja",
            "ninja",
            &["--version"],
            Expected::Any,
            "install `ninja` with Homebrew or `ninja-build` with apt".into(),
        ),
        Check::required(
            "nanopb generator",
            "nanopb_generator",
            &["--version"],
            Expected::Contains(expectations.nanopb.clone()),
            format!(
                "run `python3 -m pip install nanopb=={}` and ensure its scripts directory is on PATH",
                expectations.nanopb
            ),
        ),
        ]);
    }

    if matches!(scope, Scope::Ios) || (matches!(scope, Scope::All) && cfg!(target_os = "macos")) {
        checks.extend(ios_checks(&expectations.ios, matches!(scope, Scope::Ios)));
    }

    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    println!("Extrittio {scope:?} development environment");
    for check in checks {
        match check.evaluate(root) {
            Ok(version) => println!("  ok    {:31} {}", check.label, first_line(&version)),
            Err(reason) if check.required => {
                println!("  FAIL  {:31} {reason}", check.label);
                failures.push(format!(
                    "{}: {}; {}",
                    check.label, reason, check.remediation
                ));
            }
            Err(reason) => {
                println!("  warn  {:31} {reason}", check.label);
                warnings.push(format!(
                    "{}: {}; {}",
                    check.label, reason, check.remediation
                ));
            }
        }
    }

    if !warnings.is_empty() {
        println!("\nOptional iOS tooling:");
        for warning in warnings {
            println!("  - {warning}");
        }
    }
    if failures.is_empty() {
        println!("\nAll required development prerequisites are available.");
        Ok(())
    } else {
        bail!(
            "development environment needs attention:\n  - {}",
            failures.join("\n  - ")
        )
    }
}

fn ios_checks(versions: &HashMap<String, String>, required: bool) -> Vec<Check> {
    let make = |label, program, args, expected, remediation| {
        if required {
            Check::required(label, program, args, expected, remediation)
        } else {
            Check::optional(label, program, args, expected, remediation)
        }
    };
    vec![
        make(
            "Xcode",
            "xcodebuild",
            &["-version"],
            Expected::Any,
            "install the Xcode version required by apps/mobile-app-ios/README.md".into(),
        ),
        make(
            "XcodeGen",
            "xcodegen",
            &["--version"],
            expected_word(versions, "XCODEGEN_VERSION"),
            "install the XcodeGen version from apps/mobile-app-ios/Tools/versions.env".into(),
        ),
        make(
            "SwiftLint",
            "swiftlint",
            &["version"],
            expected_word(versions, "SWIFTLINT_VERSION"),
            "install the SwiftLint version from apps/mobile-app-ios/Tools/versions.env".into(),
        ),
        make(
            "SwiftFormat",
            "swiftformat",
            &["--version"],
            expected_word(versions, "SWIFTFORMAT_VERSION"),
            "install the SwiftFormat version from apps/mobile-app-ios/Tools/versions.env".into(),
        ),
    ]
}

fn expected_word(versions: &HashMap<String, String>, key: &str) -> Expected {
    versions
        .get(key)
        .cloned()
        .map(Expected::Word)
        .unwrap_or(Expected::Any)
}

struct Check {
    label: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    expected: Expected,
    remediation: String,
    required: bool,
}
impl Check {
    fn required(
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        expected: Expected,
        remediation: String,
    ) -> Self {
        Self {
            label,
            program,
            args,
            expected,
            remediation,
            required: true,
        }
    }

    fn optional(
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        expected: Expected,
        remediation: String,
    ) -> Self {
        Self {
            label,
            program,
            args,
            expected,
            remediation,
            required: false,
        }
    }

    fn evaluate(&self, root: &Path) -> Result<String, String> {
        let output = capture(command_in(self.program, root).args(self.args))
            .map_err(|error| format!("{error:#}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if !output.status.success() {
            let detail = if stderr.is_empty() { &stdout } else { &stderr };
            return Err(if detail.is_empty() {
                format!("`{}` exited with {}", self.program, output.status)
            } else {
                format!(
                    "`{}` exited with {}: {}",
                    self.program,
                    output.status,
                    first_line(detail)
                )
            });
        }
        let version = if stdout.is_empty() { stderr } else { stdout };
        self.expected.validate(&version)?;
        Ok(version)
    }
}

enum Expected {
    Any,
    Exact(String),
    Word(String),
    Contains(String),
    MajorAtLeast(u64),
}

impl Expected {
    fn validate(&self, actual: &str) -> Result<(), String> {
        match self {
            Self::Any if actual.is_empty() => Err("command returned no version information".into()),
            Self::Any => Ok(()),
            Self::Exact(expected) if actual.trim() == expected => Ok(()),
            Self::Exact(expected) => {
                Err(format!("expected {expected}, got {}", first_line(actual)))
            }
            Self::Word(expected) if actual.split_whitespace().any(|word| word == expected) => {
                Ok(())
            }
            Self::Word(expected) => Err(format!("expected {expected}, got {}", first_line(actual))),
            Self::Contains(expected) if actual.contains(expected) => Ok(()),
            Self::Contains(expected) => {
                Err(format!("expected {expected}, got {}", first_line(actual)))
            }
            Self::MajorAtLeast(expected) => {
                let actual_major = actual
                    .trim_start_matches(|character: char| !character.is_ascii_digit())
                    .split('.')
                    .next()
                    .and_then(|value| value.parse::<u64>().ok())
                    .ok_or_else(|| {
                        format!("could not parse version from {}", first_line(actual))
                    })?;
                if actual_major >= *expected {
                    Ok(())
                } else {
                    Err(format!(
                        "expected major version {expected} or newer, got {}",
                        first_line(actual)
                    ))
                }
            }
        }
    }
}

struct Expectations {
    rust: String,
    node_major: u64,
    npm: String,
    nanopb: String,
    ios: HashMap<String, String>,
}

impl Expectations {
    fn read(root: &Path) -> Result<Self> {
        let rust_toolchain = fs::read_to_string(root.join("rust-toolchain.toml"))
            .context("failed to read rust-toolchain.toml")?;
        let rust = quoted_value(&rust_toolchain, "channel")?;

        let package_json = fs::read_to_string(root.join("apps/frontend/package.json"))
            .context("failed to read apps/frontend/package.json")?;
        let npm = quoted_value(&package_json, "packageManager")?
            .strip_prefix("npm@")
            .context("frontend packageManager must use npm")?
            .to_owned();
        let node_major = quoted_value(&package_json, "node")?
            .trim_start_matches(|character: char| !character.is_ascii_digit())
            .split('.')
            .next()
            .context("frontend Node.js engine is empty")?
            .parse()
            .context("frontend Node.js engine does not start with a major version")?;

        let versions = fs::read_to_string(root.join("apps/mobile-app-ios/Tools/versions.env"))
            .context("failed to read iOS tool versions")?;
        let ios = versions
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| {
                (
                    key.trim().to_owned(),
                    value.trim().trim_matches('"').to_owned(),
                )
            })
            .collect();
        let nanopb_header =
            fs::read_to_string(root.join("clients/c/sdk-c/generated/telemetry.pb.h"))
                .context("failed to read generated nanopb header")?;
        let nanopb = nanopb_header
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("/* Generated by nanopb-")
                    .and_then(|value| value.strip_suffix(" */"))
            })
            .context("generated nanopb header does not identify its generator version")?
            .to_owned();
        Ok(Self {
            rust,
            node_major,
            npm,
            nanopb,
            ios,
        })
    }
}

fn quoted_value(contents: &str, key: &str) -> Result<String> {
    let quoted_key = format!("\"{key}\"");
    let value = contents
        .lines()
        .find(|line| {
            let line = line.trim_start();
            line.starts_with(&quoted_key)
                || line
                    .strip_prefix(key)
                    .is_some_and(|suffix| suffix.trim_start().starts_with('='))
        })
        .and_then(|line| line.split_once('=').or_else(|| line.split_once(':')))
        .map(|(_, value)| value.trim().trim_end_matches(',').trim_matches('"'))
        .filter(|value| !value.is_empty())
        .with_context(|| format!("could not find {key} in repository configuration"))?;
    Ok(value.to_owned())
}

fn first_line(value: &str) -> &str {
    value.lines().next().unwrap_or("unknown version")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tool_versions_from_repository_configuration() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let expected = Expectations::read(root).unwrap();
        assert_eq!(expected.rust, "1.90.0");
        assert_eq!(expected.node_major, 22);
        assert_eq!(expected.npm, "10.9.3");
        assert_eq!(expected.nanopb, "0.4.9.1");
        assert_eq!(expected.ios["XCODEGEN_VERSION"], "2.45.4");
    }

    #[test]
    fn validates_exact_word_and_minimum_major_versions() {
        assert!(Expected::Exact("10.9.3".into()).validate("10.9.3").is_ok());
        assert!(
            Expected::Word("1.90.0".into())
                .validate("rustc 1.90.0 (abc)")
                .is_ok()
        );
        assert!(
            Expected::Contains("0.4.9.1".into())
                .validate("nanopb-0.4.9.1")
                .is_ok()
        );
        assert!(Expected::MajorAtLeast(22).validate("v22.15.0").is_ok());
        assert!(Expected::MajorAtLeast(22).validate("v20.0.0").is_err());
    }
}
