use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::command::{command_in, run};

pub(crate) fn generate(root: &Path) -> Result<()> {
    generate_to(root, &root.join("clients/c/sdk-c/generated"))
}

fn generate_to(root: &Path, output_dir: &Path) -> Result<()> {
    let generator =
        std::env::var_os("NANOPB_GENERATOR").unwrap_or_else(|| "nanopb_generator".into());
    let proto_dir = root.join("crates/common/src/protos");
    let sdk_dir = root.join("clients/c/sdk-c");
    run(command_in(generator, root).args([
        format!("--proto-path={}", proto_dir.display()),
        proto_dir.join("telemetry.proto").display().to_string(),
        format!(
            "--options-file={}",
            sdk_dir.join("nanopb/telemetry.options").display()
        ),
        format!("--output-dir={}", output_dir.display()),
    ]))
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let temporary = tempfile::tempdir().context("failed to create a nanopb check directory")?;
    generate_to(root, temporary.path())?;
    let committed = root.join("clients/c/sdk-c/generated");
    for file in ["telemetry.pb.c", "telemetry.pb.h"] {
        let expected = fs::read(committed.join(file))
            .with_context(|| format!("failed to read committed nanopb binding {file}"))?;
        let actual = fs::read(temporary.path().join(file))
            .with_context(|| format!("failed to read generated nanopb binding {file}"))?;
        if expected != actual {
            bail!(
                "committed nanopb binding {file} is stale; regenerate bindings with `cargo xtask protocol generate`"
            );
        }
    }
    Ok(())
}
