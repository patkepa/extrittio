use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::command::{command_in, output, run};

pub(crate) fn bootstrap(root: &Path) -> Result<()> {
    check_tools(root)?;
    generate(root)
}

pub(crate) fn generate(root: &Path) -> Result<()> {
    run(command_in("xcodegen", &ios_root(root)).arg("generate"))
}

pub(crate) fn build(
    root: &Path,
    configuration: &str,
    scheme: &str,
    destination: &str,
) -> Result<()> {
    generate(root)?;
    run(command_in("xcodebuild", &ios_root(root)).args([
        "build",
        "-scheme",
        scheme,
        "-configuration",
        configuration,
        "-destination",
        destination,
    ]))
}

pub(crate) fn test(
    root: &Path,
    configuration: &str,
    scheme: &str,
    destination: &str,
) -> Result<()> {
    generate(root)?;
    run(command_in("xcodebuild", &ios_root(root)).args([
        "test",
        "-scheme",
        scheme,
        "-configuration",
        configuration,
        "-destination",
        destination,
    ]))
}

pub(crate) fn lint(root: &Path) -> Result<()> {
    run(command_in("swiftlint", &ios_root(root)).args(["lint", "--config", ".swiftlint.yml"]))
}

pub(crate) fn format(root: &Path, check: bool) -> Result<()> {
    let mut command = command_in("swiftformat", &ios_root(root));
    if check {
        command.arg("--lint");
    }
    command.arg(".");
    run(&mut command)
}

pub(crate) fn module_check(root: &Path) -> Result<()> {
    let ios = ios_root(root);
    let mut failures = Vec::new();
    if rg_has_matches(
        &ios,
        "^import (SwiftUI|UIKit|SwiftData|Security|MapKit|os)$",
    )? {
        failures.push("Domain must not import UI, persistence, security, or logging frameworks.");
    }
    if rg_has_matches(&ios, "APIClient|KeychainHelper|URLSession|UserDefaults")? {
        failures.push(
            "Domain must not depend on concrete data, storage, or transport implementations.",
        );
    }
    if failures.is_empty() {
        println!("Module boundary checks passed");
        Ok(())
    } else {
        bail!(failures.join("\n"))
    }
}

fn check_tools(root: &Path) -> Result<()> {
    let ios = ios_root(root);
    let versions = read_versions(&ios.join("Tools/versions.env"))?;
    let checks = [
        (
            "xcodegen",
            "XCODEGEN_VERSION",
            vec!["--version"],
            VersionShape::LastWord,
        ),
        (
            "swiftlint",
            "SWIFTLINT_VERSION",
            vec!["version"],
            VersionShape::Whole,
        ),
        (
            "swiftformat",
            "SWIFTFORMAT_VERSION",
            vec!["--version"],
            VersionShape::Whole,
        ),
    ];
    let mut failures = Vec::new();
    for (program, key, arguments, shape) in checks {
        let expected = versions
            .get(key)
            .with_context(|| format!("{key} is missing from Tools/versions.env"))?;
        let actual = match output(command_in(program, &ios).args(arguments)) {
            Ok(value) => match shape {
                VersionShape::Whole => value,
                VersionShape::LastWord => value
                    .split_whitespace()
                    .next_back()
                    .unwrap_or_default()
                    .to_owned(),
            },
            Err(error) => {
                failures.push(format!("{program}: {error:#}"));
                continue;
            }
        };
        if &actual != expected {
            failures.push(format!(
                "{program} version mismatch: expected {expected}, got {actual}"
            ));
        }
    }
    if failures.is_empty() {
        println!("All required tools match Tools/versions.env");
        Ok(())
    } else {
        bail!(
            "{}\nInstall or update tools, then rerun this command.",
            failures.join("\n")
        )
    }
}

enum VersionShape {
    Whole,
    LastWord,
}

fn read_versions(path: &Path) -> Result<HashMap<String, String>> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut versions = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("invalid version line `{line}`"))?;
        versions.insert(
            key.trim().to_owned(),
            value.trim().trim_matches('"').to_owned(),
        );
    }
    Ok(versions)
}

fn rg_has_matches(directory: &Path, pattern: &str) -> Result<bool> {
    let status = Command::new("rg")
        .args(["-n", pattern, "Extrittio/Domain"])
        .current_dir(directory)
        .status()
        .context("failed to run rg for the iOS module boundary check")?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("rg failed while checking iOS module boundaries: {status}"),
    }
}

fn ios_root(root: &Path) -> std::path::PathBuf {
    root.join("apps/mobile-app-ios")
}

#[cfg(test)]
mod tests {
    use super::read_versions;
    use std::path::Path;

    #[test]
    fn parses_ios_tool_versions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let versions = read_versions(&root.join("apps/mobile-app-ios/Tools/versions.env")).unwrap();
        assert!(versions.contains_key("XCODEGEN_VERSION"));
        assert!(versions.contains_key("SWIFTLINT_VERSION"));
        assert!(versions.contains_key("SWIFTFORMAT_VERSION"));
    }
}
