mod architecture;
mod command;
mod doctor;
mod edge;
mod ios;
mod protocol;
mod verify;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "cargo xtask",
    bin_name = "cargo xtask",
    about = "Extrittio repository build and development tasks"
)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Validate backend crate boundaries and deployment dependency closures.
    Architecture,
    /// Check whether repository development prerequisites are installed.
    Doctor,
    /// Run repository verification checks.
    Verify(VerifyArgs),
    /// Build and install an Extrittio executable.
    Install(InstallArgs),
    /// Build, set up, and deploy Extrittio Edge.
    Edge {
        #[command(subcommand)]
        command: EdgeTask,
    },
    /// Manage the pinned OpenThread Border Router build.
    Otbr {
        #[command(subcommand)]
        command: OtbrTask,
    },
    /// Build deployable Edge packages with Docker Buildx.
    Package {
        #[command(subcommand)]
        command: PackageTask,
    },
    /// Run native iOS development workflows.
    Ios {
        #[command(subcommand)]
        command: IosTask,
    },
    /// Generate or verify committed protocol bindings.
    Protocol {
        #[command(subcommand)]
        command: ProtocolTask,
    },
}

#[derive(Debug, Args)]
struct VerifyArgs {
    /// Verification scope. Defaults to all scopes unless --changed is used.
    #[arg(value_enum)]
    scope: Option<verify::VerifyScope>,

    /// Select verification scopes from tracked and untracked working-tree changes.
    #[arg(long)]
    changed: bool,
}

#[derive(Debug, Args)]
struct InstallArgs {
    /// Artifact to install.
    #[arg(value_enum, default_value_t = InstallTarget::Server)]
    target: InstallTarget,

    /// Build with Cargo's release profile.
    #[arg(long)]
    release: bool,

    /// Installation prefix containing bin/ and libexec/.
    #[arg(long, env = "EXTRITTIO_INSTALL_ROOT")]
    root: Option<PathBuf>,

    /// CMake build directory used for OTBR.
    #[arg(
        long,
        env = "OTBR_BUILD_DIR",
        default_value = "target/openthread/build"
    )]
    otbr_build_dir: PathBuf,

    /// Maximum parallel jobs used by the OTBR build.
    #[arg(long, env = "OTBR_BUILD_JOBS", default_value_t = 2)]
    otbr_jobs: usize,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum InstallTarget {
    Server,
    Edge,
    EdgeBinary,
}

#[derive(Debug, Subcommand)]
enum EdgeTask {
    /// Build frontend assets embedded by Extrittio Edge.
    Assets,
    /// Bootstrap a Raspberry Pi over SSH for Edge deployments.
    SetupPi(SetupPiArgs),
    /// Atomically deploy an Edge archive to a configured Raspberry Pi.
    DeployPi(DeployPiArgs),
}

#[derive(Debug, Args)]
struct SetupPiArgs {
    #[arg(long, default_value = "192.0.2.10")]
    host: String,

    #[arg(long, default_value = "pi")]
    user: String,

    /// SSH private key. Defaults to ~/.ssh/extrittio-pi.
    #[arg(long)]
    identity: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct DeployPiArgs {
    /// SSH target such as pi@extrittio-pi.local.
    host: String,

    /// Archive produced by `cargo xtask package edge-linux-arm64`.
    package: PathBuf,
}

#[derive(Debug, Subcommand)]
enum OtbrTask {
    /// Clone, verify, patch, configure, and build otbr-agent.
    Build(OtbrArgs),
    /// Clone or verify the pinned source revision and submodules.
    CheckSource,
}

#[derive(Debug, Args)]
struct OtbrArgs {
    #[arg(
        long,
        env = "OTBR_BUILD_DIR",
        default_value = "target/openthread/build"
    )]
    build_dir: PathBuf,

    #[arg(long, env = "OTBR_BUILD_JOBS", default_value_t = 2)]
    jobs: usize,
}

#[derive(Debug, Subcommand)]
enum PackageTask {
    /// Package Extrittio Edge and OTBR as a Linux arm64 tar archive.
    EdgeLinuxArm64(PackageArgs),
    /// Package the standalone Extrittio Edge executable as an arm64 .deb.
    EdgeDebArm64(PackageArgs),
}

#[derive(Debug, Args)]
struct PackageArgs {
    /// Output directory. Defaults depend on the package format.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Package version. Defaults to the Git revision or workspace version.
    #[arg(long, env = "EXTRITTIO_PACKAGE_VERSION")]
    version: Option<String>,
}

#[derive(Debug, Subcommand)]
enum IosTask {
    /// Validate tools and regenerate the Xcode project.
    Bootstrap,
    /// Regenerate the Xcode project.
    Generate,
    /// Build the app for an iOS Simulator.
    Build(IosBuildArgs),
    /// Run the Swift Testing suite.
    Test(IosTestArgs),
    /// Run SwiftLint.
    Lint,
    /// Apply SwiftFormat or validate formatting.
    Format {
        #[arg(long)]
        check: bool,
    },
    /// Validate Clean Architecture import and dependency boundaries.
    ModuleCheck,
}

#[derive(Debug, Args)]
struct IosBuildArgs {
    #[arg(long, value_enum, default_value_t = IosConfiguration::Debug)]
    configuration: IosConfiguration,

    #[arg(long)]
    scheme: Option<String>,

    #[arg(long, default_value = "generic/platform=iOS Simulator")]
    destination: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum IosConfiguration {
    Debug,
    Staging,
    Release,
}

impl IosConfiguration {
    fn as_xcode(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Staging => "Staging",
            Self::Release => "Release",
        }
    }

    fn default_scheme(self) -> &'static str {
        match self {
            Self::Staging => "Extrittio-Staging",
            Self::Debug | Self::Release => "Extrittio",
        }
    }
}

#[derive(Debug, Args)]
struct IosTestArgs {
    #[arg(long, default_value = "Extrittio")]
    scheme: String,

    #[arg(long, default_value = "Debug")]
    configuration: String,

    #[arg(long, default_value = "iPhone 17 Pro")]
    simulator: String,

    #[arg(long, default_value = "latest")]
    os: String,

    /// Complete xcodebuild destination, overriding simulator and OS.
    #[arg(long)]
    destination: Option<String>,
}

#[derive(Debug, Subcommand)]
enum ProtocolTask {
    /// Regenerate nanopb C bindings from the canonical protobuf.
    Generate,
    /// Regenerate bindings and fail if committed output differs.
    Check,
}

fn main() -> Result<()> {
    let root = repository_root()?;
    match Cli::parse().command {
        Task::Architecture => architecture::run(&root),
        Task::Doctor => doctor::run(&root),
        Task::Verify(args) => {
            if args.changed && args.scope.is_some() {
                anyhow::bail!("--changed cannot be combined with an explicit verification scope");
            }
            verify::run(
                &root,
                if args.changed {
                    verify::Selection::Changed
                } else {
                    verify::Selection::Scope(args.scope.unwrap_or(verify::VerifyScope::All))
                },
            )
        }
        Task::Install(args) => {
            let install_root = args.root.map_or_else(default_install_root, Ok)?;
            edge::install(
                &root,
                match args.target {
                    InstallTarget::Server => edge::InstallTarget::Server,
                    InstallTarget::Edge => edge::InstallTarget::Edge,
                    InstallTarget::EdgeBinary => edge::InstallTarget::EdgeBinary,
                },
                args.release,
                &install_root,
                &root.join(args.otbr_build_dir),
                args.otbr_jobs,
            )
        }
        Task::Edge { command } => match command {
            EdgeTask::Assets => edge::build_frontend_assets(&root),
            EdgeTask::SetupPi(args) => {
                let identity = args.identity.map_or_else(default_pi_identity, Ok)?;
                edge::setup_pi(&root, &args.host, &args.user, &identity)
            }
            EdgeTask::DeployPi(args) => edge::deploy_pi(&root, &args.host, &args.package),
        },
        Task::Otbr { command } => match command {
            OtbrTask::Build(args) => edge::build_otbr(&root, &root.join(args.build_dir), args.jobs),
            OtbrTask::CheckSource => edge::check_otbr_source(&root),
        },
        Task::Package { command } => match command {
            PackageTask::EdgeLinuxArm64(args) => {
                edge::package_linux_arm64(&root, args.output.as_deref(), args.version.as_deref())
            }
            PackageTask::EdgeDebArm64(args) => {
                edge::package_deb_arm64(&root, args.output.as_deref(), args.version.as_deref())
            }
        },
        Task::Ios { command } => match command {
            IosTask::Bootstrap => ios::bootstrap(&root),
            IosTask::Generate => ios::generate(&root),
            IosTask::Build(args) => ios::build(
                &root,
                args.configuration.as_xcode(),
                args.scheme
                    .as_deref()
                    .unwrap_or_else(|| args.configuration.default_scheme()),
                &args.destination,
            ),
            IosTask::Test(args) => {
                let destination = args.destination.unwrap_or_else(|| {
                    format!(
                        "platform=iOS Simulator,name={},OS={}",
                        args.simulator, args.os
                    )
                });
                ios::test(&root, &args.configuration, &args.scheme, &destination)
            }
            IosTask::Lint => ios::lint(&root),
            IosTask::Format { check } => ios::format(&root, check),
            IosTask::ModuleCheck => ios::module_check(&root),
        },
        Task::Protocol { command } => match command {
            ProtocolTask::Generate => protocol::generate(&root),
            ProtocolTask::Check => protocol::check(&root),
        },
    }
}

fn repository_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("xtask must live at tools/xtask inside the repository")
}

fn default_install_root() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".cargo"))
        .context("--root or EXTRITTIO_INSTALL_ROOT is required when HOME is unset")
}

fn default_pi_identity() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ssh/extrittio-pi"))
        .context("--identity is required when HOME is unset")
}
