use std::{collections::BTreeSet, time::Duration};

use anyhow::Context;
use diesel::PgConnection;
use diesel::RunQueryDsl;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::MigrationHarness;
use tracing::{debug, info, warn};

use crate::db::models::{NewDeviceType, NewServerConfigEntry, NewUser, ServerConfigEntry};
use crate::db::schema::{ca_certificates, device_types, server_config, users};
use crate::domains::identity::certificate_types::NewCaCertificateRecord;
use crate::persistence::{BootstrapOwner, BuiltinDeviceType, Persistence, SeedOwnerOutcome};
use crate::repositories::{cert_repo, role_repo, user_repo};
use crate::services::{cert_service, role_service, user_service};
use crate::state::DbPool;
use crate::{MIGRATIONS, auth};
use extrittio_common::topics::{self, patterns};

/// Create the PostgreSQL connection pool.
pub fn create_db_pool(database_url: &str, pool_size: u32) -> anyhow::Result<DbPool> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .max_size(pool_size)
        .connection_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(300)))
        .build(manager)
        .context("Failed to create database connection pool")
}

/// Run pending migrations.
pub fn run_migrations(conn: &mut PgConnection) -> anyhow::Result<()> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {e}"))?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Ensure built-in device types exist even if a dev/test database was reseeded
/// after migrations had already run.
pub fn seed_default_device_types(conn: &mut PgConnection) -> anyhow::Result<()> {
    const BUILT_IN_DEVICE_TYPES: &[(&str, &str, &str)] = &[
        ("default", "cube", "#8ABBFF"),
        ("mac-device", "desktop", "#F7C948"),
        ("network-analyzer", "antenna", "#36CFC9"),
        ("OrganBath", "heatmap", "#E76A6E"),
    ];

    let rows: Vec<NewDeviceType> = BUILT_IN_DEVICE_TYPES
        .iter()
        .map(|(name, icon, color_hex)| NewDeviceType {
            tenant_id: crate::tenancy::DEFAULT_TENANT_ID.to_string(),
            name: (*name).to_string(),
            icon: (*icon).to_string(),
            color_hex: (*color_hex).to_string(),
        })
        .collect();

    diesel::insert_into(device_types::table)
        .values(&rows)
        .on_conflict((device_types::tenant_id, device_types::name))
        .do_nothing()
        .execute(conn)
        .context("Failed to seed built-in device types")?;

    Ok(())
}

/// Initialize JWT secret from the database, or generate one if not present.
/// Falls back to the `JWT_SECRET` environment variable if set.
pub fn init_jwt_secret(conn: &mut PgConnection) -> anyhow::Result<String> {
    if let Some(secret) = std::env::var("JWT_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(secret);
    }

    if env_bool("EXTRITTIO_REQUIRE_ENV_SECRETS") {
        anyhow::bail!("JWT_SECRET must be set when EXTRITTIO_REQUIRE_ENV_SECRETS=true");
    }

    let jwt_secret = {
        let existing: Option<ServerConfigEntry> = server_config::table
            .find("jwt_secret")
            .select(ServerConfigEntry::as_select())
            .first(conn)
            .optional()
            .context("Failed to query server_config")?;

        if let Some(entry) = existing {
            entry.value
        } else {
            use rand::Rng;
            let secret: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();

            let entry = NewServerConfigEntry {
                key: "jwt_secret".to_string(),
                value: secret.clone(),
            };
            diesel::insert_into(server_config::table)
                .values(&entry)
                .execute(conn)
                .context("Failed to insert JWT secret")?;

            info!("Generated new JWT secret");
            secret
        }
    };

    Ok(jwt_secret)
}

fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Seed the first owner user if explicitly configured.
pub fn seed_admin_user(conn: &mut PgConnection) -> anyhow::Result<()> {
    let user_count: i64 = users::table
        .count()
        .get_result(conn)
        .context("Failed to count users")?;

    if user_count == 0 {
        let Some(password) = std::env::var("EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            warn!(
                "No users exist and EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD is not set; owner bootstrap skipped"
            );
            return Ok(());
        };

        user_service::validate_password(&password)
            .map_err(|e| anyhow::anyhow!("Invalid bootstrap admin password: {e}"))?;
        let username = std::env::var("EXTRITTIO_BOOTSTRAP_ADMIN_USERNAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "admin".to_string());
        anyhow::ensure!(
            password != "admin" && password != username,
            "Bootstrap admin password must not be a default or match the username"
        );

        let password_hash = auth::hash_password(&password)
            .map_err(|e| anyhow::anyhow!("Failed to hash default password: {e}"))?;
        let admin = NewUser {
            tenant_id: crate::tenancy::DEFAULT_TENANT_ID.to_string(),
            username: username.clone(),
            password_hash,
            role: role_service::OWNER_ROLE.to_string(),
        };
        let admin = user_repo::insert_user(conn, &admin).context("Failed to seed admin user")?;
        let owner_role = role_repo::find_role_by_name(
            conn,
            crate::tenancy::DEFAULT_TENANT_ID,
            role_service::OWNER_ROLE,
        )
        .context("Failed to find owner role for seeded admin")?;
        role_repo::set_user_roles(
            conn,
            crate::tenancy::DEFAULT_TENANT_ID,
            admin.id,
            &[owner_role.id],
        )
        .context("Failed to assign owner role to seeded admin")?;
        info!("Bootstrap owner user created (username: {username})");
    }
    Ok(())
}

/// Ensure the root CA certificate exists in the database.
pub fn init_ca_certificate(conn: &mut PgConnection) -> anyhow::Result<()> {
    let ca_exists: i64 = ca_certificates::table
        .count()
        .get_result(conn)
        .context("Failed to count CA certificates")?;

    if ca_exists == 0 {
        let new_ca =
            cert_service::generate_ca_certificate().context("Failed to generate CA certificate")?;
        cert_repo::insert_ca_certificate(conn, &new_ca)
            .context("Failed to insert CA certificate")?;
        info!("Generated new root CA certificate");
    }
    Ok(())
}

/// Write CA and server TLS certificates to disk for Zenoh.
pub fn write_tls_certs(conn: &mut PgConnection, certs_dir: &str) -> anyhow::Result<()> {
    let certs_path = std::path::PathBuf::from(certs_dir);
    std::fs::create_dir_all(&certs_path).context("Failed to create certs directory")?;

    let ca = cert_repo::get_ca_certificate(conn)
        .context("Failed to read CA certificate")?
        .context("CA certificate must exist")?;

    std::fs::write(certs_path.join("ca.pem"), &ca.certificate_pem)
        .context("Failed to write CA cert to disk")?;

    let server_cert_path = certs_path.join("server.pem");
    let server_key_path = certs_path.join("server-key.pem");
    if !server_cert_path.exists() || !server_key_path.exists() {
        let (server_cert_pem, server_key_pem) = cert_service::generate_server_certificate(&ca)
            .context("Failed to generate server certificate")?;
        std::fs::write(&server_cert_path, &server_cert_pem)
            .context("Failed to write server cert to disk")?;
        std::fs::write(&server_key_path, &server_key_pem)
            .context("Failed to write server key to disk")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&server_key_path, std::fs::Permissions::from_mode(0o600))
                .context("Failed to set server key permissions")?;
        }

        info!("Generated server TLS certificate");
    }

    info!("TLS certificates at {}", certs_dir);
    Ok(())
}

/// Run backend-specific migrations through the persistence facade.
pub async fn run_persistence_migrations(persistence: &Persistence) -> anyhow::Result<()> {
    persistence.bootstrap.run_migrations().await?;
    info!("Database migrations completed successfully");
    Ok(())
}

pub async fn seed_persistence_device_types(persistence: &Persistence) -> anyhow::Result<()> {
    let tenant = crate::tenancy::TenantId::new(crate::tenancy::DEFAULT_TENANT_ID)
        .expect("default tenant id is valid");
    let records = [
        ("default", "cube", "#8ABBFF"),
        ("mac-device", "desktop", "#F7C948"),
        ("network-analyzer", "antenna", "#36CFC9"),
        ("OrganBath", "heatmap", "#E76A6E"),
    ]
    .into_iter()
    .map(|(name, icon, color_hex)| BuiltinDeviceType {
        name: name.to_string(),
        icon: icon.to_string(),
        color_hex: color_hex.to_string(),
    })
    .collect();
    persistence
        .bootstrap
        .seed_builtin_device_types(&tenant, records)
        .await?;
    Ok(())
}

pub async fn init_persistence_jwt_secret(persistence: &Persistence) -> anyhow::Result<String> {
    if let Some(secret) = std::env::var("JWT_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(secret);
    }
    if env_bool("EXTRITTIO_REQUIRE_ENV_SECRETS") {
        anyhow::bail!("JWT_SECRET must be set when EXTRITTIO_REQUIRE_ENV_SECRETS=true");
    }

    use rand::Rng;
    let generated: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    persistence
        .bootstrap
        .get_or_create_server_config("jwt_secret", generated)
        .await
        .map_err(Into::into)
}

pub async fn seed_persistence_admin_user(persistence: &Persistence) -> anyhow::Result<()> {
    if persistence.bootstrap.users_exist().await? {
        return Ok(());
    }
    let Some(password) = std::env::var("EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        warn!(
            "No users exist and EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD is not set; owner bootstrap skipped"
        );
        return Ok(());
    };
    user_service::validate_password(&password)
        .map_err(|error| anyhow::anyhow!("Invalid bootstrap admin password: {error}"))?;
    let username = std::env::var("EXTRITTIO_BOOTSTRAP_ADMIN_USERNAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "admin".to_string());
    anyhow::ensure!(
        password != "admin" && password != username,
        "Bootstrap admin password must not be a default or match the username"
    );
    let password_hash = tokio::task::spawn_blocking(move || auth::hash_password(&password))
        .await
        .context("Bootstrap password task failed")?
        .map_err(|error| anyhow::anyhow!("Failed to hash default password: {error}"))?;
    let tenant = crate::tenancy::TenantId::new(crate::tenancy::DEFAULT_TENANT_ID)
        .expect("default tenant id is valid");
    let outcome = persistence
        .bootstrap
        .seed_owner_if_empty(
            &tenant,
            BootstrapOwner {
                username: username.clone(),
                password_hash,
            },
        )
        .await?;
    if outcome == SeedOwnerOutcome::Created {
        info!("Bootstrap owner user created (username: {username})");
    }
    Ok(())
}

pub async fn init_persistence_ca_certificate(persistence: &Persistence) -> anyhow::Result<()> {
    if persistence.certificates.get_ca().await?.is_some() {
        return Ok(());
    }
    let ca = tokio::task::spawn_blocking(cert_service::generate_ca_certificate)
        .await
        .context("CA generation task failed")?
        .context("Failed to generate CA certificate")?;
    persistence
        .certificates
        .insert_ca_if_absent(NewCaCertificateRecord {
            private_key_pem: ca.private_key_pem,
            certificate_pem: ca.certificate_pem,
        })
        .await?;
    info!("Generated new root CA certificate");
    Ok(())
}

pub async fn write_persistence_tls_certs(
    persistence: &Persistence,
    certs_dir: &str,
) -> anyhow::Result<()> {
    let ca = persistence
        .certificates
        .get_ca()
        .await?
        .context("CA certificate must exist")?;
    let certs_dir = certs_dir.to_string();
    tokio::task::spawn_blocking(move || {
        let legacy_ca = crate::db::models::CaCertificate {
            id: ca.id,
            private_key_pem: ca.private_key_pem,
            certificate_pem: ca.certificate_pem,
            created_at: ca.created_at.naive_utc(),
        };
        let certs_path = std::path::PathBuf::from(&certs_dir);
        std::fs::create_dir_all(&certs_path).context("Failed to create certs directory")?;
        std::fs::write(certs_path.join("ca.pem"), &legacy_ca.certificate_pem)
            .context("Failed to write CA cert to disk")?;
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");
        if !server_cert_path.exists() || !server_key_path.exists() {
            let (certificate, private_key) = cert_service::generate_server_certificate(&legacy_ca)
                .context("Failed to generate server certificate")?;
            std::fs::write(&server_cert_path, certificate)
                .context("Failed to write server cert to disk")?;
            std::fs::write(&server_key_path, private_key)
                .context("Failed to write server key to disk")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&server_key_path, std::fs::Permissions::from_mode(0o600))
                    .context("Failed to set server key permissions")?;
            }
            info!("Generated server TLS certificate");
        }
        info!("TLS certificates at {certs_dir}");
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("TLS certificate task failed")?
}

/// Configure and open a Zenoh session.
pub async fn open_zenoh_session(
    tls_enabled: bool,
    tls_port: u16,
    listen_host: &str,
    certs_dir: &str,
    cert_acl_enabled: bool,
    device_certificate_ids: &[String],
) -> anyhow::Result<zenoh::Session> {
    let mut zenoh_config = zenoh::Config::default();
    let listen_scheme = if tls_enabled { "tls" } else { "tcp" };
    let listen_endpoint = format!("{listen_scheme}/{listen_host}:{tls_port}");

    zenoh_config
        .insert_json5("scouting/multicast/enabled", "false")
        .map_err(|e| anyhow::anyhow!("Failed to disable multicast scouting: {e}"))?;

    if tls_enabled {
        let certs_path =
            std::fs::canonicalize(certs_dir).context("Failed to resolve certs directory path")?;
        let ca_path = certs_path.join("ca.pem");
        let server_cert_path = certs_path.join("server.pem");
        let server_key_path = certs_path.join("server-key.pem");

        for path in [&ca_path, &server_cert_path, &server_key_path] {
            anyhow::ensure!(path.exists(), "Missing TLS file: {}", path.display());
        }

        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh listen endpoints: {e}"))?;

        zenoh_config
            .insert_json5(
                "transport/link/tls/root_ca_certificate",
                &format!("\"{}\"", ca_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS root CA: {e}"))?;
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_certificate",
                &format!("\"{}\"", server_cert_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS server certificate: {e}"))?;
        zenoh_config
            .insert_json5(
                "transport/link/tls/listen_private_key",
                &format!("\"{}\"", server_key_path.display()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh TLS server private key: {e}"))?;
        zenoh_config
            .insert_json5("transport/link/tls/enable_mtls", "true")
            .map_err(|e| anyhow::anyhow!("Failed to enable Zenoh mTLS: {e}"))?;
        zenoh_config
            .insert_json5("transport/link/tls/close_link_on_expiration", "true")
            .map_err(|e| {
                anyhow::anyhow!("Failed to enable Zenoh TLS expiration enforcement: {e}")
            })?;

        configure_zenoh_device_acl(&mut zenoh_config, cert_acl_enabled, device_certificate_ids)?;

        info!("Zenoh TLS configured: listening on {listen_endpoint} with mTLS");
    } else {
        if cert_acl_enabled {
            warn!("Zenoh certificate ACL requested without TLS; certificate ACL is disabled");
        }
        zenoh_config
            .insert_json5("listen/endpoints", &format!("[\"{listen_endpoint}\"]"))
            .map_err(|e| anyhow::anyhow!("Failed to set Zenoh listen endpoints: {e}"))?;
        warn!("Zenoh configured: listening on {listen_endpoint} without TLS");
    }

    zenoh::open(zenoh_config).await.map_err(|e| {
        let raw_error = e.to_string();
        debug!("Raw Zenoh open error: {raw_error}");
        anyhow::anyhow!(format_zenoh_open_error(
            &raw_error,
            &listen_endpoint,
            tls_port
        ))
    })
}

fn configure_zenoh_device_acl(
    zenoh_config: &mut zenoh::Config,
    cert_acl_enabled: bool,
    device_certificate_ids: &[String],
) -> anyhow::Result<()> {
    if !cert_acl_enabled {
        warn!(
            "Zenoh device certificate ACL disabled; mTLS clients are not bound to per-device topics"
        );
        return Ok(());
    }

    let (valid_device_ids, skipped_count) = normalize_acl_device_ids(device_certificate_ids);
    if skipped_count > 0 {
        warn!(
            "Skipped {skipped_count} device certificate ID(s) with invalid topic characters while building Zenoh ACL"
        );
    }
    if valid_device_ids.is_empty() {
        warn!(
            "Zenoh device certificate ACL enabled with no active device certificates; all device traffic will be denied until the backend restarts with active certificates"
        );
    } else {
        info!(
            "Zenoh device certificate ACL enabled for {} active device certificate(s)",
            valid_device_ids.len()
        );
    }

    let acl_json = build_zenoh_device_acl(&valid_device_ids);
    zenoh_config
        .insert_json5("access_control", &acl_json)
        .map_err(|e| anyhow::anyhow!("Failed to configure Zenoh device certificate ACL: {e}"))?;
    Ok(())
}

fn normalize_acl_device_ids(device_ids: &[String]) -> (Vec<String>, usize) {
    let mut valid = BTreeSet::new();
    let mut skipped_count = 0;

    for device_id in device_ids {
        if topics::is_valid_device_id(device_id) {
            valid.insert(device_id.clone());
        } else {
            skipped_count += 1;
        }
    }

    (valid.into_iter().collect(), skipped_count)
}

fn build_zenoh_device_acl(device_ids: &[String]) -> String {
    let mut rules = Vec::new();
    let mut subjects = Vec::new();
    let mut policies = Vec::new();

    if device_ids.is_empty() {
        rules.push(serde_json::json!({
            "id": "deny-all-no-active-device-certificates",
            "permission": "deny",
            "flows": ["ingress", "egress"],
            "messages": ["put", "delete", "declare_subscriber", "query", "reply", "declare_queryable"],
            "key_exprs": ["**"],
        }));
        subjects.push(serde_json::json!({
            "id": "no-active-device-certificates",
            "cert_common_names": ["extrittio-no-active-device-certificates.invalid"],
        }));
        policies.push(serde_json::json!({
            "id": "no-active-device-certificates",
            "rules": ["deny-all-no-active-device-certificates"],
            "subjects": ["no-active-device-certificates"],
        }));
    } else {
        rules.push(serde_json::json!({
            "id": "backend-device-subscriptions",
            "permission": "allow",
            "flows": ["egress"],
            "messages": ["declare_subscriber"],
            "key_exprs": [
                patterns::TELEMETRY,
                patterns::HEARTBEAT,
                patterns::SHADOW_REPORT,
                patterns::SHADOW_GET,
                patterns::LOGS,
                patterns::COMMANDS_RESPONSE,
            ],
        }));

        for (index, device_id) in device_ids.iter().enumerate() {
            let subject_id = format!("device-{index}");
            let ingress_put_rule_id = format!("{subject_id}-ingress-put");
            let ingress_subscribe_rule_id = format!("{subject_id}-ingress-subscribe");
            let egress_put_rule_id = format!("{subject_id}-egress-put");

            subjects.push(serde_json::json!({
                "id": subject_id,
                "cert_common_names": [device_id],
            }));
            rules.push(serde_json::json!({
                "id": ingress_put_rule_id,
                "permission": "allow",
                "flows": ["ingress"],
                "messages": ["put"],
                "key_exprs": device_ingress_put_keyexprs(device_id),
            }));
            rules.push(serde_json::json!({
                "id": ingress_subscribe_rule_id,
                "permission": "allow",
                "flows": ["ingress"],
                "messages": ["declare_subscriber"],
                "key_exprs": device_downstream_keyexprs(device_id),
            }));
            rules.push(serde_json::json!({
                "id": egress_put_rule_id,
                "permission": "allow",
                "flows": ["egress"],
                "messages": ["put"],
                "key_exprs": device_downstream_keyexprs(device_id),
            }));
            policies.push(serde_json::json!({
                "id": format!("{subject_id}-policy"),
                "rules": [
                    "backend-device-subscriptions",
                    ingress_put_rule_id,
                    ingress_subscribe_rule_id,
                    egress_put_rule_id,
                ],
                "subjects": [subject_id],
            }));
        }
    }

    serde_json::json!({
        "enabled": true,
        "default_permission": "deny",
        "rules": rules,
        "subjects": subjects,
        "policies": policies,
    })
    .to_string()
}

fn device_ingress_put_keyexprs(device_id: &str) -> Vec<String> {
    vec![
        topics::telemetry(device_id),
        topics::heartbeat(device_id),
        topics::shadow_report(device_id),
        topics::shadow_get(device_id),
        topics::logs(device_id),
        topics::commands_response(device_id),
    ]
}

fn device_downstream_keyexprs(device_id: &str) -> Vec<String> {
    vec![topics::commands(device_id), topics::shadow_delta(device_id)]
}

fn format_zenoh_open_error(raw_error: &str, listen_endpoint: &str, port: u16) -> String {
    let lower = raw_error.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("os error 48")
        || lower.contains("os error 98")
    {
        return format!(
            "Zenoh port {port} is already in use. Stop the existing Extrittio/Zenoh process, or start this instance with a different port, for example: ZENOH_TLS_PORT={} extrittio",
            port.saturating_add(1)
        );
    }

    format!(
        "Failed to open Zenoh session on {listen_endpoint}. Check Zenoh configuration, TLS settings, and port availability. Set RUST_LOG=extrittio_backend=debug for the raw Zenoh error."
    )
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{build_zenoh_device_acl, format_zenoh_open_error, normalize_acl_device_ids};

    #[test]
    fn zenoh_port_conflict_error_is_actionable() {
        let raw_error = "Can not create a new TCP listener bound to tcp/0.0.0.0:7447: [0.0.0.0:7447: Address already in use (os error 48) at /registry/src/tcp.rs:52.]";

        let message = format_zenoh_open_error(raw_error, "tcp/0.0.0.0:7447", 7447);

        assert!(message.contains("Zenoh port 7447 is already in use"));
        assert!(message.contains("ZENOH_TLS_PORT=7448 extrittio"));
        assert!(!message.contains("/registry/src"));
        assert!(!message.contains(".rs:"));
    }

    #[test]
    fn generic_zenoh_error_is_sanitized() {
        let message = format_zenoh_open_error(
            "some dependency detail at /registry/src/lib.rs:12",
            "tls/0.0.0.0:7447",
            7447,
        );

        assert!(message.contains("Failed to open Zenoh session on tls/0.0.0.0:7447"));
        assert!(!message.contains("/registry/src"));
        assert!(!message.contains(".rs:"));
    }

    #[test]
    fn normalizes_acl_device_ids() {
        let (device_ids, skipped_count) = normalize_acl_device_ids(&[
            "dev-b".to_string(),
            "dev/a".to_string(),
            "dev-a".to_string(),
            "dev-a".to_string(),
        ]);

        assert_eq!(device_ids, vec!["dev-a".to_string(), "dev-b".to_string()]);
        assert_eq!(skipped_count, 1);
    }

    #[test]
    fn device_acl_binds_certificate_cn_to_device_topics() {
        let acl: Value =
            serde_json::from_str(&build_zenoh_device_acl(&["dev-001".to_string()])).unwrap();

        assert_eq!(acl["enabled"], true);
        assert_eq!(acl["default_permission"], "deny");

        let subjects = acl["subjects"].as_array().unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(
            subjects[0]["cert_common_names"],
            serde_json::json!(["dev-001"])
        );

        let rules = acl["rules"].as_array().unwrap();
        assert!(rules.iter().any(|rule| {
            rule["flows"] == serde_json::json!(["ingress"])
                && rule["messages"] == serde_json::json!(["put"])
                && rule["key_exprs"]
                    .as_array()
                    .unwrap()
                    .contains(&Value::String(
                        "extrittio/devices/dev-001/telemetry".to_string(),
                    ))
        }));
        assert!(!rules.iter().any(|rule| {
            rule["key_exprs"]
                .as_array()
                .unwrap()
                .contains(&Value::String(
                    "extrittio/devices/dev-002/telemetry".to_string(),
                ))
        }));
    }

    #[test]
    fn empty_device_acl_is_valid_and_deny_by_default() {
        let acl: Value = serde_json::from_str(&build_zenoh_device_acl(&[])).unwrap();

        assert_eq!(acl["enabled"], true);
        assert_eq!(acl["default_permission"], "deny");
        assert!(!acl["rules"].as_array().unwrap().is_empty());
        assert!(!acl["subjects"].as_array().unwrap().is_empty());
        assert!(!acl["policies"].as_array().unwrap().is_empty());
    }
}
