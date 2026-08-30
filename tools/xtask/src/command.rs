use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};

use anyhow::{Context, Result, bail};

pub(crate) fn run(command: &mut Command) -> Result<()> {
    let display = display(command);
    eprintln!("+ {display}");
    let status = command
        .status()
        .with_context(|| format!("failed to start `{display}`"))?;
    ensure_success(status, &display)
}

pub(crate) fn run_quiet(command: &mut Command) -> Result<()> {
    let display = display(command);
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to start `{display}`"))?;
    ensure_success(status, &display)
}

pub(crate) fn run_with_input(command: &mut Command, input: &[u8]) -> Result<()> {
    let display = display(command);
    eprintln!("+ {display}");
    let mut child = command
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start `{display}`"))?;
    child
        .stdin
        .take()
        .context("child process did not expose stdin")?
        .write_all(input)
        .with_context(|| format!("failed to write input to `{display}`"))?;
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for `{display}`"))?;
    ensure_success(status, &display)
}

pub(crate) fn succeeds(command: &mut Command) -> Result<bool> {
    let display = display(command);
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to start `{display}`"))?;
    Ok(status.success())
}

pub(crate) fn output(command: &mut Command) -> Result<String> {
    let display = display(command);
    let output = command
        .output()
        .with_context(|| format!("failed to start `{display}`"))?;
    ensure_success(output.status, &display)?;
    String::from_utf8(output.stdout)
        .with_context(|| format!("`{display}` returned non-UTF-8 output"))
        .map(|value| value.trim().to_owned())
}

pub(crate) fn capture(command: &mut Command) -> Result<Output> {
    let display = display(command);
    command
        .output()
        .with_context(|| format!("failed to start `{display}`"))
}

pub(crate) fn command_in(program: impl AsRef<OsStr>, directory: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(directory);
    command
}

pub(crate) fn require_program(program: &str) -> Result<()> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let found = std::env::split_paths(&path).any(|directory| {
        let candidate = directory.join(program);
        candidate.is_file()
    });
    if !found {
        bail!("required program `{program}` was not found on PATH");
    }
    Ok(())
}

fn ensure_success(status: ExitStatus, display: &str) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("`{display}` exited with {status}")
    }
}

fn display(command: &Command) -> String {
    let mut parts = Vec::with_capacity(command.get_args().len() + 1);
    parts.push(shell_word(command.get_program()));
    parts.extend(command.get_args().map(shell_word));
    parts.join(" ")
}

fn shell_word(value: &OsStr) -> String {
    let value = value.to_string_lossy();
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-._/:=+@".contains(character))
    {
        value.into_owned()
    } else {
        format!("{:?}", OsString::from(value.as_ref()))
    }
}
