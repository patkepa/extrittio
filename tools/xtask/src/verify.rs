use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;

use crate::command::{command_in, output, run as run_command};
use crate::{architecture, ios, protocol};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
pub(crate) enum VerifyScope {
    Backend,
    Frontend,
    Protocol,
    Ios,
    All,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Selection {
    Changed,
    Scope(VerifyScope),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkingDirectory {
    Root,
    Frontend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandSpec {
    label: &'static str,
    directory: WorkingDirectory,
    program: &'static str,
    args: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Architecture,
    Command(CommandSpec),
    OpenApiContract,
    FrontendApiTypes,
    ProtocolBindings,
    IosFormat,
    IosLint,
    IosModuleCheck,
    IosBuild,
    IosTest,
}

pub(crate) fn run(root: &Path, selection: Selection) -> Result<()> {
    let scopes = match selection {
        Selection::Scope(scope) => expand_scope(scope),
        Selection::Changed => {
            let paths = working_tree_paths(root)?;
            let scopes = scopes_for_paths(paths.iter().map(PathBuf::as_path));
            if scopes.is_empty() {
                println!("No verification checks are needed for the current working-tree changes.");
                return Ok(());
            }
            println!(
                "Changed files select verification scopes: {}",
                scope_names(&scopes)
            );
            scopes
        }
    };

    let steps = plan(&scopes);
    println!("Running verification scopes: {}", scope_names(&scopes));
    for (index, step) in steps.iter().enumerate() {
        println!("[{}/{}] {}", index + 1, steps.len(), step.label());
        run_step(root, *step)?;
    }
    println!("Verification passed: {}", scope_names(&scopes));
    Ok(())
}

fn run_step(root: &Path, step: Step) -> Result<()> {
    match step {
        Step::Architecture => architecture::run(root),
        Step::Command(spec) => {
            let directory = match spec.directory {
                WorkingDirectory::Root => root,
                WorkingDirectory::Frontend => &root.join("apps/frontend"),
            };
            run_command(command_in(spec.program, directory).args(spec.args))
        }
        Step::OpenApiContract => check_openapi_contract(root),
        Step::FrontendApiTypes => check_frontend_api_types(root),
        Step::ProtocolBindings => protocol::check(root),
        Step::IosFormat => ios::format(root, true),
        Step::IosLint => ios::lint(root),
        Step::IosModuleCheck => ios::module_check(root),
        Step::IosBuild => ios::build(root, "Debug", "Extrittio", "generic/platform=iOS Simulator"),
        Step::IosTest => ios::test(
            root,
            "Debug",
            "Extrittio",
            "platform=iOS Simulator,name=iPhone 17 Pro,OS=latest",
        ),
    }
}

fn check_openapi_contract(root: &Path) -> Result<()> {
    let temporary = tempfile::tempdir().context("failed to create an OpenAPI check directory")?;
    let generated = temporary.path().join("openapi.json");
    run_command(
        command_in("cargo", root)
            .args([
                "run",
                "-p",
                "extrittio-backend",
                "--features",
                "openapi",
                "--bin",
                "openapi",
                "--",
            ])
            .arg(&generated),
    )?;
    compare_generated_file(
        &root.join("api/openapi.json"),
        &generated,
        "OpenAPI contract",
        "npm run generate-api",
    )
}

fn check_frontend_api_types(root: &Path) -> Result<()> {
    let temporary = tempfile::tempdir().context("failed to create an API types check directory")?;
    let generated = temporary.path().join("openapi.ts");
    let frontend = root.join("apps/frontend");
    let generator = frontend
        .join("node_modules")
        .join(".bin")
        .join("openapi-typescript");
    run_command(
        command_in(generator, &frontend)
            .arg("../../api/openapi.json")
            .arg("-o")
            .arg(&generated),
    )?;
    compare_generated_file(
        &frontend.join("src/types/openapi.ts"),
        &generated,
        "frontend OpenAPI types",
        "npm run generate-api-types",
    )
}

fn compare_generated_file(
    committed: &Path,
    generated: &Path,
    label: &str,
    regeneration_command: &str,
) -> Result<()> {
    let committed_contents = fs::read(committed).with_context(|| {
        format!(
            "failed to read committed {label} at {}",
            committed.display()
        )
    })?;
    let generated_contents = fs::read(generated).with_context(|| {
        format!(
            "failed to read generated {label} at {}",
            generated.display()
        )
    })?;
    if committed_contents == generated_contents {
        Ok(())
    } else {
        bail!("committed {label} is stale; regenerate it with `{regeneration_command}`")
    }
}

fn working_tree_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let status = output(command_in("git", root).args([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ]))?;
    changed_paths_from_status(&status)
}

fn changed_paths_from_status(status: &str) -> Result<Vec<PathBuf>> {
    let records: Vec<&str> = status
        .split('\0')
        .filter(|record| !record.is_empty())
        .collect();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < records.len() {
        let record = records[index];
        if record.len() < 4 || record.as_bytes().get(2) != Some(&b' ') {
            bail!("could not parse git status record `{record}`");
        }
        let code = &record[..2];
        paths.push(PathBuf::from(&record[3..]));
        if code.contains('R') || code.contains('C') {
            index += 1;
            let second_path = records
                .get(index)
                .context("git status rename record is missing its second path")?;
            paths.push(PathBuf::from(second_path));
        }
        index += 1;
    }
    Ok(paths)
}

fn scopes_for_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> BTreeSet<VerifyScope> {
    let mut scopes = BTreeSet::new();
    for path in paths {
        scopes.extend(scopes_for_path(path));
    }
    scopes
}

fn scopes_for_path(path: &Path) -> BTreeSet<VerifyScope> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let path = normalized.trim_start_matches("./");
    let mut scopes = BTreeSet::new();

    if is_guidance_only(path) {
        return scopes;
    }
    if path == "Cargo.toml" || path == "Cargo.lock" || path == "rust-toolchain.toml" {
        scopes.insert(VerifyScope::Backend);
        scopes.insert(VerifyScope::Protocol);
    } else if path.starts_with("apps/frontend/") {
        scopes.insert(VerifyScope::Frontend);
    } else if path.starts_with("apps/mobile-app-ios/") {
        scopes.insert(VerifyScope::Ios);
    } else if path == "api/openapi.json" {
        scopes.insert(VerifyScope::Backend);
        scopes.insert(VerifyScope::Frontend);
    } else if path.starts_with("crates/common/") || path.starts_with("clients/rust/") {
        scopes.insert(VerifyScope::Backend);
        scopes.insert(VerifyScope::Protocol);
    } else if path.starts_with("clients/c/") || path.starts_with("clients/arduino/") {
        scopes.insert(VerifyScope::Protocol);
    } else if path.starts_with("apps/extrittio/")
        || path.starts_with("crates/backend/")
        || path.starts_with("crates/backend-core/")
        || path.starts_with("crates/backend-postgres/")
        || path.starts_with("crates/backend-turso/")
        || path.starts_with("crates/backend-adapter-tests/")
        || path.starts_with("crates/device-contract/")
        || path.starts_with("crates/openthread-runtime/")
        || path.starts_with("crates/rule-engine/")
        || path.starts_with("deploy/")
        || path.starts_with("tools/xtask/")
    {
        scopes.insert(VerifyScope::Backend);
    } else if path.starts_with(".github/") || path == ".gitignore" || path == "LICENSE" {
        // Repository metadata does not select a product verification lane.
    } else {
        scopes.extend(expand_scope(VerifyScope::All));
    }
    scopes
}

fn is_guidance_only(path: &str) -> bool {
    path == "AGENTS.md"
        || path.ends_with("/AGENTS.md")
        || path.starts_with("docs/")
        || path.starts_with(".agents/")
        || path.ends_with(".md")
}

fn expand_scope(scope: VerifyScope) -> BTreeSet<VerifyScope> {
    if scope == VerifyScope::All {
        [
            VerifyScope::Backend,
            VerifyScope::Frontend,
            VerifyScope::Protocol,
            VerifyScope::Ios,
        ]
        .into_iter()
        .collect()
    } else {
        [scope].into_iter().collect()
    }
}

fn scope_names(scopes: &BTreeSet<VerifyScope>) -> String {
    scopes
        .iter()
        .map(|scope| match scope {
            VerifyScope::Backend => "backend",
            VerifyScope::Frontend => "frontend",
            VerifyScope::Protocol => "protocol",
            VerifyScope::Ios => "ios",
            VerifyScope::All => "all",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn plan(scopes: &BTreeSet<VerifyScope>) -> Vec<Step> {
    let mut steps = Vec::new();
    if scopes.contains(&VerifyScope::Backend) {
        steps.extend(BACKEND_STEPS);
    }
    if scopes.contains(&VerifyScope::Frontend) {
        steps.extend(FRONTEND_STEPS);
    }
    if scopes.contains(&VerifyScope::Protocol) {
        steps.extend(PROTOCOL_STEPS);
    }
    if scopes.contains(&VerifyScope::Ios) {
        steps.extend(IOS_STEPS);
    }
    steps
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Self::Architecture => "check backend architecture boundaries",
            Self::Command(spec) => spec.label,
            Self::OpenApiContract => "verify generated OpenAPI contract",
            Self::FrontendApiTypes => "verify generated frontend API types",
            Self::ProtocolBindings => "verify generated nanopb bindings",
            Self::IosFormat => "check iOS formatting",
            Self::IosLint => "lint iOS sources",
            Self::IosModuleCheck => "check iOS module boundaries",
            Self::IosBuild => "build the iOS app",
            Self::IosTest => "test the iOS app",
        }
    }
}

const BACKEND_STEPS: [Step; 6] = [
    Step::Architecture,
    Step::Command(CommandSpec {
        label: "check Rust formatting",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &["fmt", "--all", "--", "--check"],
    }),
    Step::Command(CommandSpec {
        label: "lint the Rust workspace",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "clippy",
            "--workspace",
            "--exclude",
            "extrittio-macos",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    }),
    Step::Command(CommandSpec {
        label: "test the Rust workspace",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "test",
            "--workspace",
            "--exclude",
            "extrittio-macos",
            "--features",
            "extrittio-backend/openapi",
        ],
    }),
    Step::Command(CommandSpec {
        label: "test the Turso backend adapter",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "test",
            "-p",
            "extrittio-backend",
            "--no-default-features",
            "--features",
            "turso",
            "--lib",
        ],
    }),
    Step::OpenApiContract,
];

const FRONTEND_STEPS: [Step; 5] = [
    Step::Command(CommandSpec {
        label: "check frontend formatting",
        directory: WorkingDirectory::Frontend,
        program: "npm",
        args: &["run", "format:check"],
    }),
    Step::Command(CommandSpec {
        label: "lint the frontend",
        directory: WorkingDirectory::Frontend,
        program: "npm",
        args: &["run", "lint"],
    }),
    Step::Command(CommandSpec {
        label: "test the frontend",
        directory: WorkingDirectory::Frontend,
        program: "npm",
        args: &["test"],
    }),
    Step::FrontendApiTypes,
    Step::Command(CommandSpec {
        label: "type-check and build the frontend",
        directory: WorkingDirectory::Frontend,
        program: "npm",
        args: &["run", "build"],
    }),
];

const PROTOCOL_STEPS: [Step; 7] = [
    Step::Command(CommandSpec {
        label: "check extrittio-common alloc support",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "check",
            "-p",
            "extrittio-common",
            "--no-default-features",
            "--features",
            "alloc",
        ],
    }),
    Step::Command(CommandSpec {
        label: "check extrittio-sdk alloc support",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "check",
            "-p",
            "extrittio-sdk",
            "--no-default-features",
            "--features",
            "alloc",
        ],
    }),
    Step::Command(CommandSpec {
        label: "test shared Rust protocol and client crates",
        directory: WorkingDirectory::Root,
        program: "cargo",
        args: &[
            "test",
            "-p",
            "extrittio-common",
            "-p",
            "extrittio-sdk",
            "-p",
            "extrittio-client-runtime",
        ],
    }),
    Step::ProtocolBindings,
    Step::Command(CommandSpec {
        label: "configure the C SDK build",
        directory: WorkingDirectory::Root,
        program: "cmake",
        args: &["-S", "clients/c/sdk-c", "-B", "build/c-sdk", "-G", "Ninja"],
    }),
    Step::Command(CommandSpec {
        label: "build the C SDK",
        directory: WorkingDirectory::Root,
        program: "cmake",
        args: &["--build", "build/c-sdk"],
    }),
    Step::Command(CommandSpec {
        label: "test the C SDK",
        directory: WorkingDirectory::Root,
        program: "ctest",
        args: &["--test-dir", "build/c-sdk", "--output-on-failure"],
    }),
];

const IOS_STEPS: [Step; 5] = [
    Step::IosFormat,
    Step::IosLint,
    Step::IosModuleCheck,
    Step::IosBuild,
    Step::IosTest,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(paths: &[&str]) -> BTreeSet<VerifyScope> {
        scopes_for_paths(paths.iter().map(Path::new))
    }

    #[test]
    fn maps_domain_paths_to_minimum_safe_scopes() {
        assert_eq!(
            scopes(&["crates/backend/src/domains/devices.rs"]),
            [VerifyScope::Backend].into_iter().collect()
        );
        assert_eq!(
            scopes(&["apps/frontend/src/App.tsx"]),
            [VerifyScope::Frontend].into_iter().collect()
        );
        assert_eq!(
            scopes(&["apps/mobile-app-ios/Extrittio/App.swift"]),
            [VerifyScope::Ios].into_iter().collect()
        );
        assert_eq!(
            scopes(&["clients/c/sdk-c/src/extrittio.c"]),
            [VerifyScope::Protocol].into_iter().collect()
        );
    }

    #[test]
    fn maps_cross_cutting_paths_to_every_affected_scope() {
        assert_eq!(
            scopes(&["crates/common/src/protos/telemetry.proto"]),
            [VerifyScope::Backend, VerifyScope::Protocol]
                .into_iter()
                .collect()
        );
        assert_eq!(
            scopes(&["api/openapi.json"]),
            [VerifyScope::Backend, VerifyScope::Frontend]
                .into_iter()
                .collect()
        );
        assert_eq!(
            scopes(&["scripts/release.sh"]),
            expand_scope(VerifyScope::All)
        );
    }

    #[test]
    fn guidance_only_changes_do_not_select_checks() {
        assert!(scopes(&["AGENTS.md", "docs/OPERATIONS.md"]).is_empty());
    }

    #[test]
    fn parses_porcelain_status_including_renames_and_untracked_files() {
        let parsed = changed_paths_from_status(
            " M crates/backend/src/lib.rs\0R  apps/frontend/src/new.tsx\0apps/frontend/src/old.tsx\0?? clients/c/new.c\0",
        )
        .unwrap();
        assert_eq!(
            parsed,
            vec![
                PathBuf::from("crates/backend/src/lib.rs"),
                PathBuf::from("apps/frontend/src/new.tsx"),
                PathBuf::from("apps/frontend/src/old.tsx"),
                PathBuf::from("clients/c/new.c"),
            ]
        );
    }

    #[test]
    fn builds_scoped_command_plans_in_stable_order() {
        let scopes = [VerifyScope::Frontend, VerifyScope::Protocol]
            .into_iter()
            .collect();
        let steps = plan(&scopes);
        assert_eq!(steps.len(), FRONTEND_STEPS.len() + PROTOCOL_STEPS.len());
        assert_eq!(steps[0].label(), "check frontend formatting");
        assert_eq!(
            steps[FRONTEND_STEPS.len()].label(),
            "check extrittio-common alloc support"
        );
        assert_eq!(steps.last().unwrap().label(), "test the C SDK");
    }

    #[test]
    fn all_plan_contains_each_scoped_plan_once() {
        let plan = plan(&expand_scope(VerifyScope::All));
        assert_eq!(
            plan.len(),
            BACKEND_STEPS.len() + FRONTEND_STEPS.len() + PROTOCOL_STEPS.len() + IOS_STEPS.len()
        );
    }
}
