use std::process::{Child, Command};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

pub(crate) struct ChildSpec {
    pub(crate) label: &'static str,
    pub(crate) command: Command,
}

struct RunningChild {
    label: &'static str,
    child: Child,
}

pub(crate) fn run(specs: Vec<ChildSpec>) -> Result<()> {
    let stopping = Arc::new(AtomicBool::new(false));
    let signal = stopping.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .context("failed to install the Ctrl-C handler")?;

    let mut children = Vec::with_capacity(specs.len());
    for mut spec in specs {
        configure_process_group(&mut spec.command);
        eprintln!("Starting {}", spec.label);
        match spec.command.spawn() {
            Ok(child) => children.push(RunningChild {
                label: spec.label,
                child,
            }),
            Err(error) => {
                stop_all(&mut children);
                return Err(error).with_context(|| format!("failed to start {}", spec.label));
            }
        }
    }

    let result = loop {
        if stopping.load(Ordering::SeqCst) {
            eprintln!("\nStopping development services...");
            break Ok(());
        }
        let mut child_result = None;
        for running in &mut children {
            match running.child.try_wait() {
                Ok(Some(status)) if status.success() => {
                    child_result = Some(Err(anyhow::anyhow!(
                        "{} stopped unexpectedly",
                        running.label
                    )));
                }
                Ok(Some(status)) => {
                    child_result = Some(Err(anyhow::anyhow!(
                        "{} exited with {status}",
                        running.label
                    )));
                }
                Ok(None) => continue,
                Err(error) => {
                    child_result = Some(
                        Err(error).with_context(|| format!("failed to inspect {}", running.label)),
                    );
                }
            }
            break;
        }
        if let Some(result) = child_result {
            break result;
        }
        thread::sleep(Duration::from_millis(150));
    };

    stop_all(&mut children);
    result
}

fn stop_all(children: &mut [RunningChild]) {
    for running in children.iter_mut() {
        terminate(&mut running.child);
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let complete = children
            .iter_mut()
            .all(|running| running.child.try_wait().ok().flatten().is_some());
        if complete {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    for running in children.iter_mut() {
        if running.child.try_wait().ok().flatten().is_none() {
            let _ = running.child.kill();
            let _ = running.child.wait();
        }
    }
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate(child: &mut Child) {
    let process_group = -(child.id() as i32);
    // SAFETY: the child was placed in its own process group immediately before spawn.
    unsafe {
        libc::kill(process_group, libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn terminate(child: &mut Child) {
    let _ = child.kill();
}
