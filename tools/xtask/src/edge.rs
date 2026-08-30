use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail, ensure};
use tempfile::Builder;

use crate::command::{
    command_in, output, require_program, run, run_quiet, run_with_input, succeeds,
};

const OTBR_VERSION: &str = "v2026.08.0";
const OTBR_COMMIT: &str = "337711e7038d0b9c8fb46a1ce888ce7f9c4c0c35";
const OTBR_REPOSITORY: &str = "https://github.com/openthread/ot-br-posix.git";

pub(crate) enum InstallTarget {
    Server,
    Edge,
    EdgeBinary,
}

pub(crate) fn install(
    root: &Path,
    target: InstallTarget,
    release: bool,
    install_root: &Path,
    otbr_build_dir: &Path,
    otbr_jobs: usize,
) -> Result<()> {
    ensure!(otbr_jobs > 0, "--otbr-jobs must be greater than zero");

    if matches!(target, InstallTarget::Edge | InstallTarget::EdgeBinary) {
        build_frontend_assets(root)?;
    }
    if matches!(target, InstallTarget::Edge) {
        build_otbr(root, otbr_build_dir, otbr_jobs)?;
    }

    let mut cargo = command_in("cargo", root);
    cargo.args(["build", "--locked", "-p", "extrittio"]);
    if release {
        cargo.arg("--release");
    }
    if matches!(target, InstallTarget::Edge | InstallTarget::EdgeBinary) {
        cargo.args(["--no-default-features", "--features", "edge"]);
    }
    run(&mut cargo)?;

    let profile = if release { "release" } else { "debug" };
    let target_dir = cargo_target_dir(root);
    let binary = target_dir.join(profile).join("extrittio");
    let bin_dir = install_root.join("bin");
    install_executable(&binary, &bin_dir.join("extrittio"))?;

    if matches!(target, InstallTarget::Edge) {
        let otbr_agent = otbr_build_dir.join("src/agent/otbr-agent");
        install_executable(
            &otbr_agent,
            &install_root.join("libexec/extrittio/otbr-agent"),
        )?;
    }
    Ok(())
}

pub(crate) fn build_frontend_assets(root: &Path) -> Result<()> {
    let frontend = root.join("apps/frontend");
    let dependency_stamp = frontend.join("node_modules/.package-lock.json");
    let package_json = frontend.join("package.json");
    let package_lock = frontend.join("package-lock.json");

    if !is_fresh(&dependency_stamp, [&package_json, &package_lock])? {
        run(command_in("npm", root).args(["--prefix", "apps/frontend", "ci"]))?;
        ensure!(
            dependency_stamp.is_file(),
            "npm ci did not create {}",
            dependency_stamp.display()
        );
    }

    let build_stamp = frontend.join("dist/.extrittio-build-stamp");
    let mut inputs = vec![
        frontend.join("index.html"),
        frontend.join("vite.config.ts"),
        frontend.join("tsconfig.json"),
        frontend.join("tsconfig.package.json"),
        package_json,
        package_lock,
        dependency_stamp,
    ];
    collect_files(&frontend.join("src"), &mut inputs)?;
    collect_files(&frontend.join("public"), &mut inputs)?;

    if !is_fresh(&build_stamp, inputs.iter())? {
        run(command_in("npm", root).args(["--prefix", "apps/frontend", "run", "build"]))?;
        fs::write(&build_stamp, b"built by cargo xtask edge assets\n")
            .with_context(|| format!("failed to write {}", build_stamp.display()))?;
    } else {
        eprintln!("frontend assets are up to date");
    }
    Ok(())
}

pub(crate) fn setup_pi(root: &Path, host: &str, user: &str, identity: &Path) -> Result<()> {
    ensure!(valid_ssh_word(host), "invalid SSH host `{host}`");
    ensure!(
        !user.is_empty()
            && user
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character)),
        "invalid SSH user `{user}`"
    );
    ensure!(
        identity.is_file(),
        "SSH identity file does not exist: {}",
        identity.display()
    );
    for required in [
        "scripts/bootstrap-edge-pi.sh",
        "deploy/raspberry-pi/extrittio.service",
        "deploy/raspberry-pi/edge.env.example",
    ] {
        ensure!(
            root.join(required).is_file(),
            "required repository file is missing: {required}"
        );
    }
    for program in ["ssh", "scp", "ssh-add", "ssh-keygen"] {
        require_program(program)?;
    }

    let fingerprint = output(command_in("ssh-keygen", root).args(["-lf", path_text(identity)]))?
        .split_whitespace()
        .nth(1)
        .context("ssh-keygen output did not contain a fingerprint")?
        .to_owned();
    let identities = Command::new("ssh-add").arg("-l").output();
    let key_is_loaded = identities.is_ok_and(|result| {
        result.status.success()
            && String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().nth(1))
                .any(|loaded| loaded == fingerprint)
    });
    if !key_is_loaded {
        run(Command::new("ssh-add").arg(identity))?;
    }

    let target = format!("{user}@{host}");
    println!("Verifying SSH, architecture, and passwordless sudo on {target}");
    let mut verify = ssh_command(identity, &target);
    verify.arg("sudo -n true && test \"$(uname -m)\" = aarch64 && hostname");
    run(&mut verify)?;

    println!("Installing Raspberry Pi runtime prerequisites");
    let bootstrap = fs::read(root.join("scripts/bootstrap-edge-pi.sh"))?;
    let mut remote_bootstrap = ssh_command(identity, &target);
    remote_bootstrap.args(["sudo", "-n", "bash", "-s"]);
    run_with_input(&mut remote_bootstrap, &bootstrap)?;

    let mut create_stage = ssh_command(identity, &target);
    create_stage.arg("mktemp -d /tmp/extrittio-pi-setup.XXXXXX");
    let remote_stage = output(&mut create_stage)?;
    ensure!(
        remote_stage.starts_with("/tmp/extrittio-pi-setup.")
            && remote_stage
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character)),
        "remote mktemp returned an unexpected path: {remote_stage}"
    );
    let setup_result = (|| {
        let mut copy = Command::new("scp");
        add_ssh_options(&mut copy, identity);
        copy.args([
            path_text(&root.join("deploy/raspberry-pi/extrittio.service")),
            path_text(&root.join("deploy/raspberry-pi/edge.env.example")),
            &format!("{target}:{remote_stage}/"),
        ]);
        run(&mut copy)?;

        println!("Installing systemd service and initial configuration");
        let mut install = ssh_command(identity, &target);
        install.args([
            "sudo",
            "-n",
            "env",
            &format!("REMOTE_STAGE={remote_stage}"),
            "bash",
            "-s",
        ]);
        run_with_input(&mut install, SETUP_PI_SCRIPT.as_bytes())
    })();
    let mut cleanup = ssh_command(identity, &target);
    cleanup.arg(format!("rm -rf '{remote_stage}'"));
    let _ = run_quiet(&mut cleanup);
    setup_result?;

    println!("Pi bootstrap complete. Set EXTRITTIO_THREAD_RCP,");
    println!("EXTRITTIO_THREAD_INFRA_INTERFACE, and EXTRITTIO_PUBLIC_URL in");
    println!("/etc/extrittio/edge.env, then build and deploy a release.");
    Ok(())
}

pub(crate) fn deploy_pi(root: &Path, host: &str, package: &Path) -> Result<()> {
    ensure!(valid_ssh_word(host), "invalid SSH target `{host}`");
    for program in ["ssh", "scp"] {
        require_program(program)?;
    }
    let package = package
        .canonicalize()
        .with_context(|| format!("package does not exist: {}", package.display()))?;
    ensure!(
        package.is_file(),
        "package is not a file: {}",
        package.display()
    );
    let package_file = package
        .file_name()
        .and_then(|name| name.to_str())
        .context("package filename must be valid UTF-8")?;
    ensure!(
        package_file.starts_with("extrittio-edge-linux-arm64-")
            && package_file.ends_with(".tar.gz"),
        "expected an extrittio-edge-linux-arm64-*.tar.gz package"
    );
    let package_name = package_file
        .strip_suffix(".tar.gz")
        .expect("suffix was checked");
    let package_version = package_name
        .strip_prefix("extrittio-edge-linux-arm64-")
        .expect("prefix was checked");
    ensure!(
        !package_version.is_empty()
            && package_version.chars().all(|character| {
                character.is_ascii_alphanumeric() || "._-".contains(character)
            }),
        "package filename contains an invalid version"
    );
    let checksum = checksum_output(root, &package)?;
    let checksum = checksum
        .split_whitespace()
        .next()
        .context("checksum tool returned no checksum")?;
    let remote_staging = format!("/tmp/{package_file}.{}.part", std::process::id());

    run(Command::new("scp").args([path_text(&package), &format!("{host}:{remote_staging}")]))?;
    let deploy_result = {
        let mut deploy = Command::new("ssh");
        deploy.arg(host).args([
            "sudo",
            "-n",
            "env",
            &format!("PACKAGE_FILE={remote_staging}"),
            &format!("PACKAGE_NAME={package_name}"),
            &format!("EXPECTED_SHA256={checksum}"),
            "bash",
            "-s",
        ]);
        run_with_input(&mut deploy, DEPLOY_PI_SCRIPT.as_bytes())
    };
    let mut cleanup = Command::new("ssh");
    cleanup.arg(host).arg(format!("rm -f '{remote_staging}'"));
    let _ = run_quiet(&mut cleanup);
    deploy_result
}

pub(crate) fn check_otbr_source(root: &Path) -> Result<()> {
    let source_dir = otbr_source_dir(root);
    if !source_dir.join(".git").exists() {
        fs::create_dir_all(source_dir.parent().expect("OTBR source has a parent"))?;
        run(command_in("git", root).args([
            "clone",
            "--depth",
            "1",
            "--branch",
            OTBR_VERSION,
            "--recurse-submodules",
            OTBR_REPOSITORY,
            path_text(&source_dir),
        ]))?;
    }

    let revision =
        output(command_in("git", root).args(["-C", path_text(&source_dir), "rev-parse", "HEAD"]))?;
    ensure!(
        revision == OTBR_COMMIT,
        "OTBR source is at {revision}, expected {OTBR_COMMIT}; remove {} to re-clone the pinned source",
        source_dir.display()
    );
    run(command_in("git", root).args([
        "-C",
        path_text(&source_dir),
        "submodule",
        "update",
        "--init",
        "--recursive",
        "--depth",
        "1",
    ]))
}

pub(crate) fn build_otbr(root: &Path, build_dir: &Path, jobs: usize) -> Result<()> {
    ensure!(jobs > 0, "--jobs must be greater than zero");
    check_otbr_source(root)?;
    apply_otbr_patches(root)?;

    let source_dir = otbr_source_dir(root);
    let mut configure = command_in("cmake", root);
    configure.args([
        "-S",
        path_text(&source_dir),
        "-B",
        path_text(build_dir),
        "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",
        "-DOTBR_DBUS=ON",
        "-DOTBR_WEB=OFF",
        "-DOTBR_REST=ON",
        "-DOT_CHANNEL_MONITOR=ON",
        "-DOT_CHANNEL_MONITOR_AUTO_START=ON",
        "-DOTBR_NAT64=OFF",
        "-DOTBR_OT_SRP_ADV_PROXY=ON",
        "-DOTBR_OT_DISCOVERY_PROXY=ON",
        "-DOTBR_TREL=OFF",
        "-DBUILD_TESTING=OFF",
    ]);
    if cfg!(target_os = "macos") {
        configure.args([
            "-DOTBR_MDNS=mDNSResponder",
            "-DOTBR_DNSSD_PLAT=ON",
            "-DOT_MDNS=OFF",
            "-DOT_MDNS_VERBOSE=OFF",
            "-DOTBR_BACKBONE_ROUTER=OFF",
            "-DOT_FIREWALL=OFF",
            "-DCMAKE_C_FLAGS=-Wno-error=uninitialized-const-pointer",
        ]);
    }
    run(&mut configure)?;
    run(command_in("cmake", root).args([
        "--build",
        path_text(build_dir),
        "--target",
        "otbr-agent",
        "--parallel",
        &jobs.to_string(),
    ]))?;

    let agent = build_dir.join("src/agent/otbr-agent");
    ensure!(
        agent.is_file(),
        "OTBR build did not create {}",
        agent.display()
    );
    Ok(())
}

pub(crate) fn package_linux_arm64(
    root: &Path,
    output_dir: Option<&Path>,
    version: Option<&str>,
) -> Result<()> {
    let version = version
        .map(str::to_owned)
        .unwrap_or(output(command_in("git", root).args([
            "rev-parse",
            "--short=12",
            "HEAD",
        ]))?);
    ensure!(
        !version.is_empty()
            && version
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character)),
        "version may contain only letters, numbers, dot, underscore, and hyphen"
    );
    require_buildx(root)?;

    let output_dir = resolve_output(root, output_dir, "dist/raspberry-pi")?;
    let package_name = format!("extrittio-edge-linux-arm64-{version}");
    let archive_name = format!("{package_name}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");
    let archive_path = output_dir.join(&archive_name);
    let checksum_path = output_dir.join(&checksum_name);
    refuse_overwrite([&archive_path, &checksum_path])?;

    let build_output = Builder::new().prefix("extrittio-edge-package.").tempdir()?;
    docker_buildx(
        root,
        "deploy/raspberry-pi/Dockerfile",
        &version,
        build_output.path(),
    )?;
    let package_dir = build_output.path().join(&package_name);
    ensure!(
        package_dir.join("bin/extrittio").is_file()
            && package_dir.join("libexec/extrittio/otbr-agent").is_file(),
        "Docker build did not produce the complete Extrittio Edge package"
    );

    run(command_in("tar", root).args([
        "-C",
        path_text(build_output.path()),
        "-czf",
        path_text(&archive_path),
        &package_name,
    ]))?;
    write_checksum(root, &archive_path, &checksum_path)?;
    println!("Created {}", archive_path.display());
    println!("Created {}", checksum_path.display());
    Ok(())
}

pub(crate) fn package_deb_arm64(
    root: &Path,
    output_dir: Option<&Path>,
    version: Option<&str>,
) -> Result<()> {
    let version = match version {
        Some(version) => version.to_owned(),
        None => format!(
            "{}+git{}",
            workspace_version(root)?,
            output(command_in("git", root).args(["rev-parse", "--short=12", "HEAD"]))?
        ),
    };
    ensure!(
        version.starts_with(|character: char| character.is_ascii_digit())
            && version.chars().all(|character| {
                character.is_ascii_alphanumeric() || ".+~:-".contains(character)
            }),
        "version must be a valid Debian version beginning with a digit"
    );
    require_buildx(root)?;

    let output_dir = resolve_output(root, output_dir, "dist/debian")?;
    let package_name = format!("extrittio_{version}_arm64.deb");
    let checksum_name = format!("{package_name}.sha256");
    let package_path = output_dir.join(&package_name);
    let checksum_path = output_dir.join(&checksum_name);
    refuse_overwrite([&package_path, &checksum_path])?;

    let build_output = Builder::new().prefix("extrittio-edge-deb.").tempdir()?;
    docker_buildx(
        root,
        "deploy/debian/Dockerfile",
        &version,
        build_output.path(),
    )?;
    let built_package = build_output.path().join(&package_name);
    let built_checksum = build_output.path().join(&checksum_name);
    ensure!(
        built_package.is_file() && built_checksum.is_file(),
        "Docker build did not produce the Debian package and checksum"
    );
    fs::copy(&built_package, &package_path)?;
    fs::copy(&built_checksum, &checksum_path)?;
    println!("Created {}", package_path.display());
    println!("Created {}", checksum_path.display());
    Ok(())
}

fn apply_otbr_patches(root: &Path) -> Result<()> {
    let mut patches = vec![
        "patches/otbr-dbus-channel-monitor-config.patch",
        "patches/otbr-dbus-channel-monitor-introspection.patch",
        "patches/otbr-rest-bearer-auth.patch",
    ];
    if cfg!(target_os = "macos") {
        patches.splice(
            0..0,
            [
                "patches/otbr-macos-ipv6-bound-if.patch",
                "patches/otbr-macos-dnssd-link.patch",
            ],
        );
    }

    let source_dir = otbr_source_dir(root);
    for patch in patches {
        let patch = root.join(patch);
        let mut check = command_in("git", root);
        check.args([
            "-C",
            path_text(&source_dir),
            "apply",
            "--check",
            path_text(&patch),
        ]);
        if succeeds(&mut check)? {
            run(command_in("git", root).args([
                "-C",
                path_text(&source_dir),
                "apply",
                path_text(&patch),
            ]))?;
            continue;
        }

        let mut reverse_check = command_in("git", root);
        reverse_check.args([
            "-C",
            path_text(&source_dir),
            "apply",
            "--reverse",
            "--check",
            path_text(&patch),
        ]);
        ensure!(
            succeeds(&mut reverse_check)?,
            "{} can be neither applied nor recognized as already applied",
            patch.display()
        );
    }
    Ok(())
}

fn docker_buildx(root: &Path, dockerfile: &str, version: &str, output_dir: &Path) -> Result<()> {
    run(command_in("docker", root).args([
        "buildx",
        "build",
        "--progress=plain",
        "--platform",
        "linux/arm64",
        "--file",
        dockerfile,
        "--target",
        "artifact",
        "--build-arg",
        &format!("PACKAGE_VERSION={version}"),
        "--output",
        &format!("type=local,dest={}", output_dir.display()),
        ".",
    ]))
}

fn require_buildx(root: &Path) -> Result<()> {
    require_program("docker")?;
    run_quiet(command_in("docker", root).args(["buildx", "version"]))
        .context("Docker Buildx is required")
}

fn resolve_output(root: &Path, requested: Option<&Path>, default: &str) -> Result<PathBuf> {
    let path = match requested {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => std::env::current_dir()?.join(path),
        None => root.join(default),
    };
    fs::create_dir_all(&path)
        .with_context(|| format!("failed to create output directory {}", path.display()))?;
    Ok(path)
}

fn refuse_overwrite<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> Result<()> {
    for path in paths {
        if path.exists() {
            bail!(
                "refusing to overwrite existing artifact: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn write_checksum(root: &Path, artifact: &Path, destination: &Path) -> Result<()> {
    let checksum = checksum_output(root, artifact)?;
    fs::write(destination, format!("{checksum}\n"))
        .with_context(|| format!("failed to write {}", destination.display()))
}

fn checksum_output(root: &Path, artifact: &Path) -> Result<String> {
    if require_program("shasum").is_ok() {
        output(command_in("shasum", root).args(["-a", "256", path_text(artifact)]))
    } else {
        output(command_in("sha256sum", root).arg(path_text(artifact)))
    }
}

fn ssh_command(identity: &Path, target: &str) -> Command {
    let mut command = Command::new("ssh");
    add_ssh_options(&mut command, identity);
    command.arg(target);
    command
}

fn add_ssh_options(command: &mut Command, identity: &Path) {
    command.args(["-i", path_text(identity)]).args([
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "PasswordAuthentication=no",
    ]);
}

fn valid_ssh_word(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_:@[]%".contains(character))
}

fn workspace_version(root: &Path) -> Result<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let mut in_workspace_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_workspace_package = line == "[workspace.package]";
        } else if in_workspace_package
            && let Some(version) = line.strip_prefix("version = \"")
            && let Some(version) = version.strip_suffix('"')
        {
            return Ok(version.to_owned());
        }
    }
    bail!("workspace package version is missing from Cargo.toml")
}

fn install_executable(source: &Path, destination: &Path) -> Result<()> {
    ensure!(
        source.is_file(),
        "build did not create {}",
        source.display()
    );
    let parent = destination
        .parent()
        .context("install target has no parent")?;
    run(Command::new("install").args(["-d", path_text(parent)]))?;
    run(Command::new("install").args(["-m", "0755", path_text(source), path_text(destination)]))
}

fn cargo_target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || root.join("target"),
        |value| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        },
    )
}

fn otbr_source_dir(root: &Path) -> PathBuf {
    root.join("target/openthread/ot-br-posix")
}

fn is_fresh<'a>(target: &Path, inputs: impl IntoIterator<Item = &'a PathBuf>) -> Result<bool> {
    let Ok(target_modified) = modified(target) else {
        return Ok(false);
    };
    for input in inputs {
        if modified(input)? > target_modified {
            return Ok(false);
        }
    }
    Ok(true)
}

fn modified(path: &Path) -> Result<SystemTime> {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .with_context(|| format!("failed to read modification time for {}", path.display()))
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap_or_else(|| {
        panic!(
            "xtask currently requires UTF-8 repository paths: {}",
            path.display()
        )
    })
}

const SETUP_PI_SCRIPT: &str = r#"set -eu
install -m 0644 "$REMOTE_STAGE/extrittio.service" /etc/systemd/system/extrittio.service
if test -e /etc/extrittio/edge.env; then
  echo 'Keeping existing /etc/extrittio/edge.env unchanged.'
elif test -e /etc/extrittio/hobby.env; then
  mv /etc/extrittio/hobby.env /etc/extrittio/edge.env
  echo 'Migrated the existing configuration to /etc/extrittio/edge.env.'
else
  install -m 0600 "$REMOTE_STAGE/edge.env.example" /etc/extrittio/edge.env
  echo 'Created /etc/extrittio/edge.env; configure it before starting the service.'
fi
systemctl daemon-reload
systemctl enable extrittio.service
echo
echo 'Detected RCP candidates and network interfaces:'
ls -l /dev/serial/by-id/ 2>/dev/null || true
ip -br link
"#;

const DEPLOY_PI_SCRIPT: &str = r#"set -euo pipefail

if [[ "$(uname -m)" != "aarch64" ]]; then
  echo "refusing to deploy a Linux arm64 package to $(uname -m)" >&2
  exit 1
fi

actual_checksum="$(sha256sum "$PACKAGE_FILE" | awk '{print $1}')"
if [[ "$actual_checksum" != "$EXPECTED_SHA256" ]]; then
  echo "uploaded package checksum does not match the local build" >&2
  exit 1
fi

stage_dir="/opt/extrittio/releases/.${PACKAGE_NAME}.staging.$$"
release_dir="/opt/extrittio/releases/$PACKAGE_NAME"
cleanup() {
  rm -rf "$stage_dir"
}
trap cleanup EXIT

if [[ -e "$release_dir" ]]; then
  echo "release already exists: $release_dir" >&2
  exit 1
fi

install -d -m 0755 /opt/extrittio/releases
install -d -m 0755 "$stage_dir"
tar -xzf "$PACKAGE_FILE" -C "$stage_dir" --strip-components=1
test -x "$stage_dir/bin/extrittio"
test -x "$stage_dir/libexec/extrittio/otbr-agent"
(cd "$stage_dir" && sha256sum -c SHA256SUMS)
if ldd "$stage_dir/libexec/extrittio/otbr-agent" | grep -q 'not found'; then
  echo "otbr-agent has missing runtime libraries; run the Edge Pi bootstrap first" >&2
  exit 1
fi

mv "$stage_dir" "$release_dir"
ln -s "releases/$PACKAGE_NAME" /opt/extrittio/.current.new
mv -Tf /opt/extrittio/.current.new /opt/extrittio/current
systemctl daemon-reload
systemctl restart extrittio.service

for _ in $(seq 1 20); do
  if curl -fsS http://127.0.0.1:8080/health >/dev/null; then
    echo "deployed $PACKAGE_NAME"
    exit 0
  fi
  sleep 1
done

systemctl --no-pager --full status extrittio.service >&2 || true
echo "service did not become healthy; the release remains active for diagnosis" >&2
exit 1
"#;

#[cfg(test)]
mod tests {
    use super::{valid_ssh_word, workspace_version};
    use std::path::Path;

    #[test]
    fn reads_workspace_version() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        assert_eq!(workspace_version(root).unwrap(), "0.1.0");
    }

    #[test]
    fn validates_shell_safe_ssh_targets() {
        assert!(valid_ssh_word("pi@extrittio-pi.local"));
        assert!(valid_ssh_word("user@[fe80::1%en0]"));
        assert!(!valid_ssh_word("-oProxyCommand=bad"));
        assert!(!valid_ssh_word("pi@host;bad"));
        assert!(!valid_ssh_word(""));
    }
}
