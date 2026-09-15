use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use clap::Args;

use crate::command::{command_in, output, require_program, run, succeeds};
use crate::edge::ensure_frontend_dependencies;
use crate::supervisor::{self, ChildSpec};

const DEFAULT_BACKEND_PORT: u16 = 8080;
const DEFAULT_FRONTEND_PORT: u16 = 5173;
const DEFAULT_DATABASE_PORT: u16 = 5432;
const DEFAULT_ZENOH_PORT: u16 = 17_447;
const DEV_COMPOSE_PROJECT: &str = "extrittio-dev";
const TEST_COMPOSE_PROJECT: &str = "extrittio-test";

#[derive(Debug, Args)]
pub(crate) struct CommonArgs {
    /// HTTP port used by the Rust backend.
    #[arg(long, default_value_t = DEFAULT_BACKEND_PORT)]
    port: u16,

    /// HTTP port used by the Vite development server.
    #[arg(long, default_value_t = DEFAULT_FRONTEND_PORT)]
    frontend_port: u16,

    /// Zenoh TCP port used by local devices.
    #[arg(long, default_value_t = DEFAULT_ZENOH_PORT)]
    zenoh_port: u16,

    /// Start only the Rust backend and its database dependencies.
    #[arg(long, conflicts_with = "frontend_only")]
    backend_only: bool,

    /// Start only Vite and connect it to an already-running backend.
    #[arg(long, conflicts_with = "backend_only")]
    frontend_only: bool,

    /// Rust tracing filter for the backend process.
    #[arg(long, default_value = "extrittio=info,extrittio_backend=info")]
    log: String,
}

#[derive(Debug, Args)]
pub(crate) struct EdgeArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Enable local OpenThread discovery and supervision.
    #[arg(long)]
    thread: bool,

    /// Edge data directory. Defaults to target/xtask/edge-dev/data.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct CloudArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Host port exposed by the development PostgreSQL container.
    #[arg(long, default_value_t = DEFAULT_DATABASE_PORT)]
    database_port: u16,

    /// Delete and recreate the development PostgreSQL volume before startup.
    #[arg(long)]
    fresh: bool,

    /// Confirm deletion requested by --fresh.
    #[arg(long, requires = "fresh")]
    yes: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CloudTestArgs {
    /// Host port exposed by the development PostgreSQL container.
    #[arg(long, default_value_t = DEFAULT_DATABASE_PORT)]
    database_port: u16,
}

pub(crate) fn run_edge(root: &Path, args: EdgeArgs) -> Result<()> {
    preflight(root, &args.common, false)?;
    let data_dir = args
        .data_dir
        .unwrap_or_else(|| root.join("target/xtask/edge-dev/data"));
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;

    let mut specs = Vec::new();
    if !args.common.frontend_only {
        let mut backend = command_in("cargo", root);
        backend.args([
            "run",
            "--locked",
            "--package",
            "extrittio",
            "--no-default-features",
            "--features",
            "edge-runtime",
            "--",
            "run",
            "--no-ui",
            "--port",
            &args.common.port.to_string(),
            "--zenoh-port",
            &args.common.zenoh_port.to_string(),
            "--thread-enabled",
            if args.thread { "true" } else { "false" },
            "--data-dir",
        ]);
        backend.arg(&data_dir);
        backend.env("RUST_LOG", &args.common.log);
        specs.push(ChildSpec {
            label: "Edge backend",
            command: backend,
        });
    }
    add_frontend(root, &args.common, &mut specs, true);

    print_summary("Edge", &args.common, Some(&data_dir), "admin / admin");
    supervisor::run(specs)
}

pub(crate) fn run_cloud(root: &Path, args: CloudArgs) -> Result<()> {
    ensure!(
        !args.fresh || args.yes,
        "--fresh deletes the development PostgreSQL volume; rerun with --fresh --yes"
    );
    ensure!(
        !args.fresh || !args.common.frontend_only,
        "--fresh cannot be used with --frontend-only"
    );
    preflight(root, &args.common, !args.common.frontend_only)?;

    let work_dir = root.join("target/xtask/cloud-dev");
    std::fs::create_dir_all(&work_dir)
        .with_context(|| format!("failed to create {}", work_dir.display()))?;
    let mut postgres_started_here = false;
    if !args.common.frontend_only {
        postgres_started_here =
            start_postgres(root, args.database_port, args.fresh, DEV_COMPOSE_PROJECT)?;
    }

    let mut specs = Vec::new();
    if !args.common.frontend_only {
        let database_url = format!(
            "postgres://extrittio:extrittio@127.0.0.1:{}/extrittio",
            args.database_port
        );
        let frontend_url = format!("http://localhost:{}", args.common.frontend_port);
        let backend_url = format!("http://localhost:{}", args.common.port);
        let mut backend = command_in("cargo", root);
        backend.args([
            "run",
            "--locked",
            "--package",
            "extrittio",
            "--no-default-features",
            "--features",
            "postgres",
            "--",
            "serve",
            "--no-ui",
            "--port",
            &args.common.port.to_string(),
            "--zenoh-tls-port",
            &args.common.zenoh_port.to_string(),
        ]);
        backend
            .env("DATABASE_URL", database_url)
            .env("CORS_ORIGIN", frontend_url)
            .env("EXTRITTIO_PUBLIC_URL", backend_url)
            .env("EXTRITTIO_BOOTSTRAP_ADMIN_USERNAME", "admin")
            .env("EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD", "Extrittio-dev1!")
            .env("EXTRITTIO_CERTS_DIR", work_dir.join("certs"))
            .env("FIRMWARE_STORAGE_PATH", work_dir.join("firmware"))
            .env("RUST_LOG", &args.common.log);
        specs.push(ChildSpec {
            label: "Cloud backend",
            command: backend,
        });
    }
    add_frontend(root, &args.common, &mut specs, false);

    print_summary("Cloud", &args.common, None, "admin / Extrittio-dev1!");
    let result = supervisor::run(specs);
    if postgres_started_here {
        let cleanup = run(compose(root, args.database_port).args(["stop", "postgres"]));
        if result.is_ok() {
            cleanup?;
        }
    }
    result
}

pub(crate) fn test_edge(root: &Path) -> Result<()> {
    for program in ["cargo", "protoc"] {
        require_program(program)?;
    }
    run(command_in("cargo", root).args([
        "test",
        "--locked",
        "--package",
        "extrittio",
        "--no-default-features",
        "--features",
        "edge-runtime",
    ]))?;
    run(command_in("cargo", root).args([
        "test",
        "--locked",
        "--package",
        "extrittio-backend-adapter-tests",
        "--features",
        "turso",
        "--",
        "--test-threads=1",
    ]))
}

pub(crate) fn test_cloud(root: &Path, args: CloudTestArgs) -> Result<()> {
    for program in ["cargo", "docker", "protoc"] {
        require_program(program)?;
    }
    let postgres_started_here =
        start_postgres(root, args.database_port, false, TEST_COMPOSE_PROJECT)?;
    let database_url = format!(
        "postgres://extrittio:extrittio@127.0.0.1:{}/extrittio",
        args.database_port
    );

    let mut app_tests = command_in("cargo", root);
    app_tests.args([
        "test",
        "--locked",
        "--package",
        "extrittio",
        "--no-default-features",
        "--features",
        "postgres",
    ]);
    app_tests.env("DATABASE_URL", &database_url);
    let app_result = run(&mut app_tests);

    let adapter_result = if app_result.is_ok() {
        let mut adapter_tests = command_in("cargo", root);
        adapter_tests.args([
            "test",
            "--locked",
            "--package",
            "extrittio-backend-adapter-tests",
            "--features",
            "postgres",
            "--",
            "--test-threads=1",
        ]);
        adapter_tests.env("DATABASE_URL", database_url);
        run(&mut adapter_tests)
    } else {
        Ok(())
    };

    let cleanup_result = if postgres_started_here {
        run(
            compose_project(root, args.database_port, TEST_COMPOSE_PROJECT)
                .args(["down", "--volumes"]),
        )
    } else {
        Ok(())
    };
    app_result?;
    adapter_result?;
    cleanup_result
}

fn preflight(root: &Path, args: &CommonArgs, docker: bool) -> Result<()> {
    if !args.frontend_only {
        for program in ["cargo", "protoc"] {
            require_program(program)?;
        }
    }
    if !args.backend_only {
        for program in ["node", "npm"] {
            require_program(program)?;
        }
    }
    if docker {
        require_program("docker")?;
    }
    if !args.frontend_only {
        check_port(args.port, "backend")?;
        check_port(args.zenoh_port, "Zenoh")?;
    }
    if !args.backend_only {
        check_port(args.frontend_port, "frontend")?;
        ensure_frontend_dependencies(root)?;
    }
    Ok(())
}

fn check_port(port: u16, label: &str) -> Result<()> {
    let flag = match label {
        "backend" => "--port",
        "frontend" => "--frontend-port",
        "database" => "--database-port",
        _ => "--zenoh-port",
    };
    TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .map(|_| ())
        .map_err(|_| anyhow::anyhow!("{label} port {port} is unavailable; pass a different {flag}"))
}

fn add_frontend(root: &Path, args: &CommonArgs, specs: &mut Vec<ChildSpec>, edge: bool) {
    if args.backend_only {
        return;
    }
    let backend_url = format!("http://localhost:{}", args.port);
    let mut frontend = command_in("npm", root);
    frontend.args([
        "--prefix",
        "apps/frontend",
        "run",
        "dev",
        "--",
        "--host",
        "127.0.0.1",
        "--port",
        &args.frontend_port.to_string(),
        "--strictPort",
    ]);
    frontend.env("EXTRITTIO_DEV_API_URL", backend_url);
    if edge {
        frontend.env("EXTRITTIO_DEPLOYMENT_PROFILE", "edge");
    }
    specs.push(ChildSpec {
        label: "Vite frontend",
        command: frontend,
    });
}

fn print_summary(runtime: &str, args: &CommonArgs, data_dir: Option<&Path>, account: &str) {
    eprintln!("\nExtrittio {runtime} development");
    if !args.backend_only {
        eprintln!("  Web UI:   http://localhost:{}", args.frontend_port);
    }
    if !args.frontend_only {
        eprintln!("  API:      http://localhost:{}", args.port);
        eprintln!("  Zenoh:    tcp/127.0.0.1:{}", args.zenoh_port);
        eprintln!("  Account:  {account}");
    }
    if let Some(data_dir) = data_dir {
        eprintln!("  Data:     {}", data_dir.display());
    }
    eprintln!("  Stop:     Ctrl-C\n");
}

fn compose(root: &Path, database_port: u16) -> Command {
    compose_project(root, database_port, DEV_COMPOSE_PROJECT)
}

fn compose_project(root: &Path, database_port: u16, project: &str) -> Command {
    let mut command = command_in("docker", root);
    command.args([
        "compose",
        "--project-name",
        project,
        "--file",
        "deploy/docker/docker-compose.yml",
    ]);
    command.env("POSTGRES_PORT", database_port.to_string());
    command
}

fn verify_docker(root: &Path, database_port: u16, project: &str) -> Result<()> {
    run(compose_project(root, database_port, project).args(["version"]))?;
    let mut info = command_in("docker", root);
    info.arg("info");
    ensure!(
        succeeds(&mut info)?,
        "Docker is installed but its daemon is unavailable; start Docker and retry"
    );
    Ok(())
}

fn start_postgres(root: &Path, database_port: u16, fresh: bool, project: &str) -> Result<bool> {
    verify_docker(root, database_port, project)?;
    if fresh {
        run(compose_project(root, database_port, project).args(["down", "--volumes"]))?;
    }
    let already_running = postgres_running(root, database_port, project)?;
    if !already_running {
        check_port(database_port, "database")?;
    }
    if let Err(error) =
        run(compose_project(root, database_port, project).args(["up", "-d", "postgres"]))
    {
        if !already_running {
            let _ = run(compose_project(root, database_port, project).args(["stop", "postgres"]));
        }
        return Err(error);
    }
    let started_here = !already_running;
    if let Err(error) = wait_for_postgres(root, database_port, project) {
        if started_here {
            let _ = run(compose_project(root, database_port, project).args(["stop", "postgres"]));
        }
        return Err(error);
    }
    Ok(started_here)
}

fn postgres_running(root: &Path, database_port: u16, project: &str) -> Result<bool> {
    let services = output(compose_project(root, database_port, project).args([
        "ps",
        "--status",
        "running",
        "--services",
        "postgres",
    ]))?;
    Ok(services.lines().any(|service| service.trim() == "postgres"))
}

fn wait_for_postgres(root: &Path, database_port: u16, project: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if succeeds(compose_project(root, database_port, project).args([
            "exec",
            "-T",
            "postgres",
            "pg_isready",
            "-U",
            "extrittio",
            "-d",
            "extrittio",
        ]))? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    bail!(
        "PostgreSQL did not become ready within 30 seconds; inspect it with `docker compose --project-name {project} -f deploy/docker/docker-compose.yml logs postgres`"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_are_distinct() {
        assert_ne!(DEFAULT_BACKEND_PORT, DEFAULT_FRONTEND_PORT);
        assert_ne!(DEFAULT_BACKEND_PORT, DEFAULT_DATABASE_PORT);
        assert_ne!(DEFAULT_BACKEND_PORT, DEFAULT_ZENOH_PORT);
    }

    #[test]
    fn compose_command_is_isolated_from_other_projects() {
        let command = compose(Path::new("/tmp/repository"), 55432);
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--project-name", DEV_COMPOSE_PROJECT])
        );
        assert_eq!(
            command.get_envs().find_map(|(key, value)| {
                (key == "POSTGRES_PORT").then(|| value.unwrap().to_string_lossy().into_owned())
            }),
            Some("55432".into())
        );
    }
}
