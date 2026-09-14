use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::command::{command_in, output};

const CORE_PACKAGE: &str = "extrittio-backend-core";
const HOST_PACKAGE: &str = "extrittio-backend";

const PACKAGE_RULES: &[PackageRule] = &[
    PackageRule {
        package: "extrittio",
        allowed_workspace_dependencies: &[HOST_PACKAGE],
    },
    PackageRule {
        package: HOST_PACKAGE,
        allowed_workspace_dependencies: &[
            CORE_PACKAGE,
            "extrittio-backend-postgres",
            "extrittio-backend-turso",
            "extrittio-common",
            "extrittio-device-contract",
            "extrittio-openthread-runtime",
        ],
    },
    PackageRule {
        package: CORE_PACKAGE,
        allowed_workspace_dependencies: &[
            "extrittio-common",
            "extrittio-device-contract",
            "extrittio-rule-engine",
        ],
    },
    PackageRule {
        package: "extrittio-backend-postgres",
        allowed_workspace_dependencies: &[CORE_PACKAGE],
    },
    PackageRule {
        package: "extrittio-backend-turso",
        allowed_workspace_dependencies: &[CORE_PACKAGE],
    },
    PackageRule {
        package: "extrittio-backend-adapter-tests",
        allowed_workspace_dependencies: &[
            CORE_PACKAGE,
            "extrittio-backend-postgres",
            "extrittio-backend-turso",
        ],
    },
];

const CORE_FORBIDDEN_DEPENDENCIES: &[&str] = &[
    "argon2",
    "axum",
    "diesel",
    "diesel_migrations",
    "dotenvy",
    "jsonwebtoken",
    "libsql",
    "mdns-sd",
    "object_store",
    "opentelemetry",
    "opentelemetry-otlp",
    "opentelemetry_sdk",
    "prost",
    "rcgen",
    "reqwest",
    "ring",
    "sysinfo",
    "tokio",
    "tower",
    "tower-http",
    "turso",
    "zenoh",
];

const CORE_FORBIDDEN_SOURCE_TOKENS: &[&str] = &[
    "argon2::",
    "axum::",
    "diesel::",
    "dotenvy::",
    "extrittio_common::extrittio",
    "jsonwebtoken::",
    "libsql::",
    "object_store::",
    "prost::",
    "rcgen::",
    "reqwest::",
    "ring::",
    "std::env",
    "tokio::",
    "tower::",
    "turso::",
    "zenoh::",
];

const ADAPTER_FORBIDDEN_SOURCE_TOKENS: &[&str] = &[
    "axum::",
    "crate::http",
    "dotenvy::",
    "extrittio_backend::",
    "extrittio_openthread",
    "jsonwebtoken::",
    "std::env",
    "zenoh::",
];

const DEFAULT_TENANT_ALLOWANCES: &[TokenAllowance] = &[
    TokenAllowance::new(
        "crates/backend/src/tenancy.rs",
        "DEFAULT_TENANT_ID",
        1,
        "P1.2",
        "legacy/default bootstrap identifier definition",
    ),
    TokenAllowance::new(
        "crates/backend/src/init.rs",
        "DEFAULT_TENANT_ID",
        1,
        "P1.2/P3.1",
        "current application bootstrap mapping",
    ),
    TokenAllowance::new(
        "crates/backend/src/auth/context.rs",
        "DEFAULT_TENANT_ID",
        3,
        "P1.2",
        "legacy claim compatibility must move to the host mapper",
    ),
    TokenAllowance::new(
        "crates/backend/src/auth.rs",
        "DEFAULT_TENANT_ID",
        2,
        "P1.2",
        "legacy token generation/claims behavior",
    ),
    TokenAllowance::new(
        "crates/backend/src/middleware.rs",
        "DEFAULT_TENANT_ID",
        2,
        "P1.2",
        "audit middleware currently invents tenant scope",
    ),
    TokenAllowance::new(
        "crates/backend/src/persistence/turso/mod.rs",
        "DEFAULT_TENANT_ID",
        4,
        "P3.1",
        "inline adapter test fixtures",
    ),
    TokenAllowance::new(
        "crates/backend/src/domains/identity/auth_routes.rs",
        "DEFAULT_TENANT_ID",
        2,
        "P1.2",
        "legacy authentication route compatibility",
    ),
];

/// Exact P3 migration debt for HTTP handlers that still bypass `Application`.
///
/// Both field counts are capped independently so changing `state.persistence`
/// to `state.repositories` cannot disguise a new direct repository access.
/// Entries and caps may only be removed or reduced as vertical slices migrate.
const APP_STATE_REPOSITORY_ACCESS_ALLOWANCES: &[AppStateRepositoryAccessAllowance] = &[
    AppStateRepositoryAccessAllowance::new("crates/backend/src/domains/audit/audit.rs", 1, 0),
    AppStateRepositoryAccessAllowance::new(
        "crates/backend/src/domains/operations/server_metrics_api.rs",
        2,
        0,
    ),
    AppStateRepositoryAccessAllowance::new(
        "crates/backend/src/domains/operations/server_metrics_service.rs",
        1,
        0,
    ),
    AppStateRepositoryAccessAllowance::new("crates/backend/src/middleware.rs", 1, 0),
];

const APP_TURSO_ALLOWANCES: &[TokenAllowance] = &[TokenAllowance::new(
    "apps/extrittio/src/commands/service.rs",
    "TursoDatabase",
    4,
    "P5.3",
    "current CLI performs Turso maintenance directly",
)];

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    dependencies: Vec<CargoDependency>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
    path: Option<PathBuf>,
    #[serde(default)]
    optional: bool,
    uses_default_features: bool,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Clone, Copy)]
struct PackageRule {
    package: &'static str,
    allowed_workspace_dependencies: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct TokenAllowance {
    path: &'static str,
    token: &'static str,
    maximum_occurrences: usize,
    removal_work_package: &'static str,
    reason: &'static str,
}

#[derive(Clone, Copy)]
struct AppStateRepositoryAccessAllowance {
    path: &'static str,
    persistence: usize,
    repositories: usize,
}

impl AppStateRepositoryAccessAllowance {
    const fn new(path: &'static str, persistence: usize, repositories: usize) -> Self {
        Self {
            path,
            persistence,
            repositories,
        }
    }
}

impl TokenAllowance {
    const fn new(
        path: &'static str,
        token: &'static str,
        maximum_occurrences: usize,
        removal_work_package: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            path,
            token,
            maximum_occurrences,
            removal_work_package,
            reason,
        }
    }
}

#[derive(Default)]
struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Report {
    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    fn finish(self) -> Result<()> {
        for warning in &self.warnings {
            eprintln!("architecture warning: {warning}");
        }
        if self.errors.is_empty() {
            println!(
                "Architecture checks passed with {} tracked migration exception(s).",
                self.warnings.len()
            );
            Ok(())
        } else {
            bail!(
                "architecture checks failed with {} violation(s):\n- {}",
                self.errors.len(),
                self.errors.join("\n- ")
            )
        }
    }
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let metadata = load_metadata(root)?;
    let mut report = Report::default();

    check_package_graph(root, &metadata, &mut report);
    check_source_boundaries(root, &mut report)?;
    check_dependency_closure(
        root,
        "production",
        "production",
        &["libsql", "libsql-sys", "turso"],
        &mut report,
    )?;
    check_dependency_closure(
        root,
        "edge",
        "edge",
        &["diesel", "diesel_derives", "diesel_migrations", "pq-sys"],
        &mut report,
    )?;

    report.finish()
}

fn load_metadata(root: &Path) -> Result<CargoMetadata> {
    let json =
        output(command_in("cargo", root).args(["metadata", "--format-version", "1", "--no-deps"]))?;
    serde_json::from_str(&json).context("failed to parse cargo metadata for architecture checks")
}

fn check_package_graph(root: &Path, metadata: &CargoMetadata, report: &mut Report) {
    let packages: BTreeMap<&str, &CargoPackage> = metadata
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect();

    for rule in PACKAGE_RULES {
        let Some(package) = packages.get(rule.package).copied() else {
            println!(
                "architecture: planned package `{}` is not present yet; its dependency rule is armed",
                rule.package
            );
            continue;
        };

        for dependency in package
            .dependencies
            .iter()
            .filter(|dependency| is_workspace_dependency(root, dependency))
        {
            if rule
                .allowed_workspace_dependencies
                .contains(&dependency.name.as_str())
            {
                continue;
            }
            if let Some((work_package, reason)) =
                legacy_workspace_edge(rule.package, &dependency.name)
            {
                report.warning(format!(
                    "{} -> {} is a bounded legacy edge ({work_package}: {reason})",
                    rule.package, dependency.name
                ));
            } else {
                report.error(format!(
                    "{} must not depend on workspace package {}",
                    rule.package, dependency.name
                ));
            }
        }
    }

    if let Some(core) = packages.get(CORE_PACKAGE).copied() {
        check_core_dependencies(core, report);
    }

    check_migration_bridge_features(&packages, report);

    if let Some(host) = packages.get(HOST_PACKAGE).copied() {
        let defaults = host.features.get("default").map_or(&[][..], Vec::as_slice);
        if !defaults.is_empty() {
            let p2_packages_exist = [
                CORE_PACKAGE,
                "extrittio-backend-postgres",
                "extrittio-backend-turso",
            ]
            .iter()
            .all(|package| packages.contains_key(package));
            if p2_packages_exist {
                report.error(format!(
                    "{HOST_PACKAGE} default features must be empty after P2 scaffolding; found {defaults:?}"
                ));
            } else {
                report.warning(format!(
                    "{HOST_PACKAGE} still defaults to {defaults:?} (remove in P2.1)"
                ));
            }
        }
    }
}

fn check_migration_bridge_features(packages: &BTreeMap<&str, &CargoPackage>, report: &mut Report) {
    const ADAPTER_PACKAGES: &[&str] = &["extrittio-backend-postgres", "extrittio-backend-turso"];
    const BRIDGE_FEATURE: &str = "migration-bridge";

    for adapter_name in ADAPTER_PACKAGES {
        let Some(adapter) = packages.get(adapter_name).copied() else {
            continue;
        };

        if !adapter.features.contains_key(BRIDGE_FEATURE) {
            report.error(format!(
                "{adapter_name} must declare the temporary `{BRIDGE_FEATURE}` feature while legacy host repositories remain"
            ));
        }
        if adapter
            .features
            .get("default")
            .is_some_and(|features| features.iter().any(|feature| feature == BRIDGE_FEATURE))
        {
            report.error(format!(
                "{adapter_name} must keep `{BRIDGE_FEATURE}` disabled by default"
            ));
        }

        let mut host_enables_bridge = false;
        for package in packages.values().copied() {
            for dependency in package
                .dependencies
                .iter()
                .filter(|dependency| dependency.name == *adapter_name)
                .filter(|dependency| {
                    dependency
                        .features
                        .iter()
                        .any(|feature| feature == BRIDGE_FEATURE)
                })
            {
                if package.name != HOST_PACKAGE {
                    report.error(format!(
                        "{} must not enable {adapter_name}/{BRIDGE_FEATURE}; the temporary bridge is host-only",
                        package.name
                    ));
                    continue;
                }
                host_enables_bridge = true;
                if !dependency.optional {
                    report.error(format!(
                        "{HOST_PACKAGE} must keep its {adapter_name}/{BRIDGE_FEATURE} dependency optional"
                    ));
                }
            }
        }

        if !host_enables_bridge {
            report.error(format!(
                "{HOST_PACKAGE} must explicitly enable {adapter_name}/{BRIDGE_FEATURE} until the legacy repositories are removed"
            ));
        }
    }
}

fn is_workspace_dependency(root: &Path, dependency: &CargoDependency) -> bool {
    dependency
        .path
        .as_deref()
        .is_some_and(|path| path.starts_with(root))
}

fn legacy_workspace_edge(package: &str, dependency: &str) -> Option<(&'static str, &'static str)> {
    match (package, dependency) {
        ("extrittio", "extrittio-openthread-runtime") => Some((
            "P5.3",
            "OpenThread composition still lives partly in the process shell",
        )),
        _ => None,
    }
}

fn check_core_dependencies(core: &CargoPackage, report: &mut Report) {
    for dependency in &core.dependencies {
        if CORE_FORBIDDEN_DEPENDENCIES.contains(&dependency.name.as_str()) {
            report.error(format!(
                "{CORE_PACKAGE} has forbidden direct dependency {}",
                dependency.name
            ));
        }
        if dependency.name == "extrittio-common" {
            if dependency.uses_default_features {
                report
                    .error("extrittio-backend-core must disable extrittio-common default features");
            }
            if !dependency.features.iter().any(|feature| feature == "alloc") {
                report.error("extrittio-backend-core must enable extrittio-common/alloc");
            }
            if dependency.features.iter().any(|feature| feature == "std") {
                report.error("extrittio-backend-core must not enable extrittio-common/std");
            }
        }
    }
}

fn check_source_boundaries(root: &Path, report: &mut Report) -> Result<()> {
    let backend_files = rust_files(root, Path::new("crates/backend"))?;
    let app_files = rust_files(root, Path::new("apps/extrittio"))?;

    check_bounded_token(
        &backend_files,
        "DEFAULT_TENANT_ID",
        DEFAULT_TENANT_ALLOWANCES,
        report,
    );
    check_bounded_token(&backend_files, "pub persistence:", &[], report);
    check_app_state_repository_access(
        &backend_files,
        APP_STATE_REPOSITORY_ACCESS_ALLOWANCES,
        report,
    );
    check_bounded_token(&backend_files, "pub repositories:", &[], report);
    check_bounded_token(&app_files, "TursoDatabase", APP_TURSO_ALLOWANCES, report);

    check_forbidden_tokens(
        &app_files,
        "process shell",
        &[
            "PostgresAdapter",
            "TursoAdapter",
            "extrittio_backend_postgres",
            "extrittio_backend_turso",
        ],
        report,
    );

    check_approved_host_adapter_imports(&backend_files, report);

    for (directory, label) in [
        ("crates/backend-core", "backend core"),
        ("crates/backend-postgres", "PostgreSQL adapter"),
        ("crates/backend-turso", "Turso adapter"),
    ] {
        let files = rust_files(root, Path::new(directory))?;
        let forbidden = if directory == "crates/backend-core" {
            CORE_FORBIDDEN_SOURCE_TOKENS
        } else {
            ADAPTER_FORBIDDEN_SOURCE_TOKENS
        };
        check_forbidden_tokens(&files, label, forbidden, report);
    }

    Ok(())
}

fn check_bounded_token(
    files: &BTreeMap<String, String>,
    token: &str,
    allowances: &[TokenAllowance],
    report: &mut Report,
) {
    for (path, contents) in files {
        let occurrences = contents.matches(token).count();
        if occurrences == 0 {
            continue;
        }
        let allowance = allowances
            .iter()
            .find(|allowance| allowance.path == path && allowance.token == token);
        match allowance {
            Some(allowance) if occurrences <= allowance.maximum_occurrences => {
                report.warning(format!(
                    "{path} contains {occurrences} occurrence(s) of `{token}` ({}: {}; maximum {})",
                    allowance.removal_work_package,
                    allowance.reason,
                    allowance.maximum_occurrences
                ));
            }
            Some(allowance) => report.error(format!(
                "{path} contains {occurrences} occurrence(s) of `{token}`, exceeding the migration allowance of {} ({})",
                allowance.maximum_occurrences, allowance.removal_work_package
            )),
            None => report.error(format!(
                "{path} contains unapproved architecture token `{token}`"
            )),
        }
    }
}

fn check_app_state_repository_access(
    files: &BTreeMap<String, String>,
    allowances: &[AppStateRepositoryAccessAllowance],
    report: &mut Report,
) {
    let mut remaining_accesses = 0;
    let mut remaining_files = 0;

    for (path, contents) in files {
        if !is_handler_source(path) {
            continue;
        }

        let persistence = count_field_access(contents, ".persistence");
        let repositories = count_field_access(contents, ".repositories");
        if persistence == 0 && repositories == 0 {
            continue;
        }

        let Some(allowance) = allowances.iter().find(|allowance| allowance.path == path) else {
            report.error(format!(
                "handler source {path} directly accesses AppState persistence/repositories without a P3 migration allowance"
            ));
            continue;
        };

        let mut within_caps = true;
        for (field, actual, maximum) in [
            ("persistence", persistence, allowance.persistence),
            ("repositories", repositories, allowance.repositories),
        ] {
            if actual > maximum {
                within_caps = false;
                report.error(format!(
                    "handler source {path} contains {actual} direct `.{field}` access(es), exceeding its P3 migration cap of {maximum}"
                ));
            }
        }

        if within_caps {
            remaining_accesses += persistence + repositories;
            remaining_files += 1;
        }
    }

    if remaining_accesses > 0 {
        report.warning(format!(
            "{remaining_accesses} direct AppState handler-to-repository access(es) remain across {remaining_files} file(s) (P3 migration debt; per-file caps may only decrease)"
        ));
    }
}

fn is_handler_source(path: &str) -> bool {
    path.starts_with("crates/backend/src/domains/")
        || path.starts_with("crates/backend/src/http/")
        || matches!(
            path,
            "crates/backend/src/app/http.rs"
                | "crates/backend/src/middleware.rs"
                | "crates/backend/src/rate_limit.rs"
        )
}

fn count_field_access(contents: &str, token: &str) -> usize {
    contents
        .match_indices(token)
        .filter(|(offset, _)| {
            contents[offset + token.len()..]
                .chars()
                .next()
                .is_none_or(|character| !(character.is_alphanumeric() || character == '_'))
        })
        .count()
}

fn check_forbidden_tokens(
    files: &BTreeMap<String, String>,
    layer: &str,
    forbidden_tokens: &[&str],
    report: &mut Report,
) {
    for (path, contents) in files {
        for token in forbidden_tokens {
            if contains_source_token(contents, token) {
                report.error(format!(
                    "{layer} source {path} contains forbidden token `{token}`"
                ));
            }
        }
    }
}

fn contains_source_token(contents: &str, token: &str) -> bool {
    contents.match_indices(token).any(|(offset, _)| {
        contents[..offset]
            .chars()
            .next_back()
            .is_none_or(|character| !(character.is_alphanumeric() || character == '_'))
    })
}

fn check_approved_host_adapter_imports(files: &BTreeMap<String, String>, report: &mut Report) {
    const TOKENS: &[&str] = &["extrittio_backend_postgres", "extrittio_backend_turso"];
    const APPROVED_PREFIXES: &[&str] = &[
        "crates/backend/src/boot/",
        "crates/backend/src/database/",
        "crates/backend/src/maintenance/",
        "crates/backend/tests/",
    ];

    for (path, contents) in files {
        for token in TOKENS {
            if contents.contains(token)
                && !APPROVED_PREFIXES
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
            {
                report.error(format!(
                    "host source {path} imports concrete adapter `{token}` outside an approved composition/maintenance/test module"
                ));
            }
        }
    }
}

fn rust_files(root: &Path, relative_directory: &Path) -> Result<BTreeMap<String, String>> {
    let directory = root.join(relative_directory);
    if !directory.exists() {
        return Ok(BTreeMap::new());
    }
    let mut paths = Vec::new();
    collect_rust_paths(&directory, &mut paths)?;
    paths.sort();

    let mut files = BTreeMap::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("{} is outside repository root", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read architecture source {}", path.display()))?;
        files.insert(relative, contents);
    }
    Ok(files)
}

fn collect_rust_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read directory {}", directory.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to inspect entry in {}", directory.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_rust_paths(&entry.path(), paths)?;
        } else if file_type.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("rs")
        {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn check_dependency_closure(
    root: &Path,
    profile: &str,
    features: &str,
    forbidden_packages: &[&str],
    report: &mut Report,
) -> Result<()> {
    let tree = output(command_in("cargo", root).args([
        "tree",
        "-p",
        "extrittio",
        "--no-default-features",
        "--features",
        features,
        "-e",
        "normal",
        "--prefix",
        "none",
    ]))?;
    let packages = package_names_from_tree(&tree);
    for forbidden in forbidden_packages {
        if packages.contains(*forbidden) {
            report.error(format!(
                "{profile} dependency closure unexpectedly contains `{forbidden}`"
            ));
        }
    }
    Ok(())
}

fn package_names_from_tree(tree: &str) -> BTreeSet<&str> {
    tree.lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_names_from_flat_cargo_tree() {
        let names = package_names_from_tree(
            "extrittio v0.1.0 (/repo/apps/extrittio)\ndiesel v2.2.0\nserde v1.0.0 (*)",
        );
        assert_eq!(
            names,
            ["diesel", "extrittio", "serde"].into_iter().collect()
        );
    }

    #[test]
    fn bounded_tokens_reject_new_paths_and_growth() {
        let allowance = [TokenAllowance::new(
            "allowed.rs",
            "TOKEN",
            1,
            "P1",
            "legacy",
        )];
        let files = BTreeMap::from([
            ("allowed.rs".to_string(), "TOKEN TOKEN".to_string()),
            ("new.rs".to_string(), "TOKEN".to_string()),
        ]);
        let mut report = Report::default();
        check_bounded_token(&files, "TOKEN", &allowance, &mut report);
        assert_eq!(report.errors.len(), 2);
    }

    #[test]
    fn app_state_repository_access_rejects_new_files_and_growth() {
        let allowances = [AppStateRepositoryAccessAllowance::new(
            "crates/backend/src/domains/allowed.rs",
            1,
            0,
        )];
        let files = BTreeMap::from([
            (
                "crates/backend/src/domains/allowed.rs".to_string(),
                "state.persistence.users(); state.persistence.roles(); state.repositories();"
                    .to_string(),
            ),
            (
                "crates/backend/src/http/new_handler.rs".to_string(),
                "state.persistence.devices();".to_string(),
            ),
        ]);
        let mut report = Report::default();

        check_app_state_repository_access(&files, &allowances, &mut report);

        assert_eq!(report.errors.len(), 3);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("direct `.persistence`") && error.contains("cap of 1"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("direct `.repositories`") && error.contains("cap of 0"))
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("new_handler.rs") && error.contains("without"))
        );
    }

    #[test]
    fn app_state_repository_access_allows_reductions_with_one_aggregate_warning() {
        let allowances = [AppStateRepositoryAccessAllowance::new(
            "crates/backend/src/domains/allowed.rs",
            2,
            1,
        )];
        let files = BTreeMap::from([(
            "crates/backend/src/domains/allowed.rs".to_string(),
            "state\n    .persistence\n    .users();".to_string(),
        )]);
        let mut report = Report::default();

        check_app_state_repository_access(&files, &allowances, &mut report);

        assert!(report.errors.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("1 direct AppState"));
        assert!(report.warnings[0].contains("across 1 file(s)"));
    }

    #[test]
    fn app_state_repository_access_uses_field_boundaries() {
        assert_eq!(
            count_field_access("state.persistence.users", ".persistence"),
            1
        );
        assert_eq!(
            count_field_access("state.persistence_error", ".persistence"),
            0
        );
    }

    #[test]
    fn identifies_only_known_legacy_workspace_edges() {
        assert!(legacy_workspace_edge("extrittio", "extrittio-openthread-runtime").is_some());
        assert!(legacy_workspace_edge(CORE_PACKAGE, HOST_PACKAGE).is_none());
    }

    #[test]
    fn source_token_matching_respects_identifier_boundaries() {
        assert!(contains_source_token("use ring::digest;", "ring::"));
        assert!(!contains_source_token("String::from(\"ok\")", "ring::"));
    }
}
