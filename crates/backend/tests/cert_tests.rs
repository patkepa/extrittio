#![cfg(feature = "postgres")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

use extrittio_backend::db::models::NewCaCertificate;
use extrittio_backend::domains::identity::certificate_types::{
    CaCertificateRecord, NewCaCertificateRecord,
};
use extrittio_backend::repositories::cert_repo;
use extrittio_backend::services::cert_service;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/postgres");
const DEFAULT_TEST_DATABASE_URL: &str =
    "postgres://extrittio:extrittio@127.0.0.1:5432/extrittio?connect_timeout=2";
const TEST_BLUEPRINT_ID: &str = "cert-test-blueprint";
const TEST_BLUEPRINT_REVISION_ID: &str = "cert-test-blueprint-r1";
static TEST_DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn insert_generated_ca(conn: &mut PgConnection, ca: NewCaCertificateRecord) -> CaCertificateRecord {
    let stored = cert_repo::insert_ca_certificate(
        conn,
        &NewCaCertificate {
            private_key_pem: ca.private_key_pem,
            certificate_pem: ca.certificate_pem,
        },
    )
    .unwrap();
    CaCertificateRecord {
        id: stored.id,
        private_key_pem: stored.private_key_pem,
        certificate_pem: stored.certificate_pem,
        created_at: stored.created_at.and_utc(),
    }
}

fn test_context() -> extrittio_backend::auth::context::RequestContext {
    extrittio_backend::auth::context::RequestContext::from_claims(extrittio_backend::auth::Claims {
        sub: 1,
        username: "admin".to_string(),
        role: "admin".to_string(),
        tenant_id: Some(extrittio_backend::tenancy::DEFAULT_TENANT_ID.to_string()),
        scopes: Vec::new(),
        permission_version: 1,
        exp: 0,
    })
}

fn setup_test_db() -> Pool<ConnectionManager<PgConnection>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_TEST_DATABASE_URL.to_string());
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    let pool = Pool::builder()
        .max_size(1)
        .connection_timeout(Duration::from_secs(3))
        .build(manager)
        .expect("Failed to create test DB pool");

    let mut conn = pool.get().unwrap();
    conn.run_pending_migrations(MIGRATIONS).unwrap();

    // Clean all data for test isolation (truncate in dependency order)
    diesel::sql_query("TRUNCATE TABLE device_contract_assignments, device_contracts, device_blueprint_revisions, device_blueprint_drafts, device_blueprints, rule_cooldowns, alerts, rule_actions, rule_conditions, rules, zones, app_metrics, server_metrics, device_certificates, ca_certificates, command_history, device_logs, ota_deployments, firmware_blobs, firmware_updates, api_keys, device_configs, device_shadows, telemetry, devices, fleets, device_types, users, server_config CASCADE")
        .execute(&mut conn)
        .unwrap();

    // Re-seed device types with explicit IDs to match test expectations
    diesel::sql_query(
        "INSERT INTO device_types (id, name) VALUES (1, 'default') ON CONFLICT DO NOTHING",
    )
    .execute(&mut conn)
    .unwrap();
    diesel::sql_query(
        "INSERT INTO device_types (id, name) VALUES (2, 'mac-device') ON CONFLICT DO NOTHING",
    )
    .execute(&mut conn)
    .unwrap();
    diesel::sql_query("SELECT setval('device_types_id_seq', 2)")
        .execute(&mut conn)
        .unwrap();
    let blueprint_document = serde_json::json!({
        "apiVersion": "extrittio.io/v1alpha1",
        "kind": "DeviceBlueprint",
        "metadata": {"key": "cert-test", "name": "Certificate test"},
        "spec": {
            "runtime": {
                "minimumContractApi": 1,
                "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
            }
        }
    });
    diesel::sql_query(
        "INSERT INTO device_blueprints (id, tenant_id, blueprint_key, name)
         VALUES ($1, 'default', 'cert-test', 'Certificate test')",
    )
    .bind::<diesel::sql_types::Text, _>(TEST_BLUEPRINT_ID)
    .execute(&mut conn)
    .unwrap();
    diesel::sql_query(
        "INSERT INTO device_blueprint_revisions
            (id, tenant_id, blueprint_id, revision, document, document_hash, compatibility)
         VALUES ($1, 'default', $2, 1, $3::jsonb, $4, '{}'::jsonb)",
    )
    .bind::<diesel::sql_types::Text, _>(TEST_BLUEPRINT_REVISION_ID)
    .bind::<diesel::sql_types::Text, _>(TEST_BLUEPRINT_ID)
    .bind::<diesel::sql_types::Text, _>(blueprint_document.to_string())
    .bind::<diesel::sql_types::Text, _>("0".repeat(64))
    .execute(&mut conn)
    .unwrap();

    pool
}

async fn setup_app_with_ca() -> (axum::Router, Pool<ConnectionManager<PgConnection>>) {
    extrittio_backend::init::install_crypto_provider();
    let db_pool = setup_test_db();
    let zenoh_session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open test zenoh session");

    // Seed CA certificate
    {
        let mut conn = db_pool.get().unwrap();
        let ca = cert_service::generate_ca_certificate().unwrap();
        insert_generated_ca(&mut conn, ca);
    }

    let state = Arc::new(extrittio_backend::state::AppState {
        persistence: extrittio_backend::persistence::postgres::create_persistence(db_pool.clone()),
        zenoh_session: Arc::new(zenoh_session),
        zenoh_tls_enabled: false,
        zenoh_port: 7447,
        jwt_secret: "test-secret-key".to_string(),
        public_url: "http://localhost:8080".to_string(),
        cookie_secure: false,
        health_token: None,
        api_rate_limiter: extrittio_backend::rate_limit::RateLimiter::new(10000, 60),
        login_rate_limiter: extrittio_backend::rate_limit::RateLimiter::new(10000, 60),
        trusted_proxies: extrittio_backend::rate_limit::parse_trusted_proxies(&[]),
        ci_rate_limiter: extrittio_backend::rate_limit::ApiKeyRateLimiter::new(10000, 60),
        metrics_accumulator: extrittio_backend::state::MetricsAccumulator::new(),
        zenoh_metrics: Arc::new(extrittio_backend::state::ZenohMetrics::new()),
        rule_cache: Arc::new(std::sync::RwLock::new(
            extrittio_backend::rule_engine::cache::RuleCache::default(),
        )),
        http_client: reqwest::Client::new(),
        firmware_store: extrittio_backend::domains::firmware_store::FirmwareObjectStore::in_memory(
        ),
        readiness: Arc::new(extrittio_backend::state::ReadinessRegistry::new(true, true)),
        thread_runtime: None,
    });

    let router = extrittio_backend::api::router(100 * 1024 * 1024, true)
        .layer(axum::Extension(test_context()))
        .with_state(state);
    (router, db_pool)
}

// ---------------------------------------------------------------------------
// Unit tests: cert_service
// ---------------------------------------------------------------------------

#[test]
fn test_generate_ca_certificate() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let ca = cert_service::generate_ca_certificate().unwrap();

    assert!(
        ca.certificate_pem
            .starts_with("-----BEGIN CERTIFICATE-----")
    );
    assert!(
        ca.private_key_pem
            .starts_with(concat!("-----BEGIN PRIVATE ", "KEY-----"))
    );

    // Fingerprint should be computable from the generated cert
    let fingerprint = cert_service::fingerprint_from_pem(&ca.certificate_pem).unwrap();
    assert!(fingerprint.contains(':'));
    // SHA-256 fingerprint = 32 bytes = 64 hex chars + 31 colons = 95 chars
    assert_eq!(fingerprint.len(), 95);
}

#[test]
fn test_generate_device_certificate() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let pool = setup_test_db();
    let mut conn = pool.get().unwrap();

    // Generate and store CA
    let new_ca = cert_service::generate_ca_certificate().unwrap();
    let ca = insert_generated_ca(&mut conn, new_ca);

    // Generate device cert
    let device_id = "test-device-001";
    let device_cert = cert_service::generate_device_certificate(device_id, &ca).unwrap();

    assert_eq!(device_cert.device_id, device_id);
    assert!(
        device_cert
            .certificate_pem
            .starts_with("-----BEGIN CERTIFICATE-----")
    );
    assert!(
        device_cert
            .private_key_pem
            .starts_with(concat!("-----BEGIN PRIVATE ", "KEY-----"))
    );
    assert!(device_cert.fingerprint.contains(':'));
    assert_eq!(device_cert.fingerprint.len(), 95);

    // Verify the cert PEM fingerprint matches the stored fingerprint
    let computed_fp = cert_service::fingerprint_from_pem(&device_cert.certificate_pem).unwrap();
    assert_eq!(device_cert.fingerprint, computed_fp);
}

#[test]
fn test_generate_server_certificate() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let pool = setup_test_db();
    let mut conn = pool.get().unwrap();

    let new_ca = cert_service::generate_ca_certificate().unwrap();
    let ca = insert_generated_ca(&mut conn, new_ca);

    let (server_cert_pem, server_key_pem) = cert_service::generate_server_certificate(&ca).unwrap();

    assert!(server_cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(server_key_pem.starts_with(concat!("-----BEGIN PRIVATE ", "KEY-----")));
}

#[test]
fn test_different_devices_get_different_certs() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let pool = setup_test_db();
    let mut conn = pool.get().unwrap();

    let new_ca = cert_service::generate_ca_certificate().unwrap();
    let ca = insert_generated_ca(&mut conn, new_ca);

    let cert1 = cert_service::generate_device_certificate("device-aaa", &ca).unwrap();
    let cert2 = cert_service::generate_device_certificate("device-bbb", &ca).unwrap();

    // Different devices should get different keys and certs
    assert_ne!(cert1.private_key_pem, cert2.private_key_pem);
    assert_ne!(cert1.certificate_pem, cert2.certificate_pem);
    assert_ne!(cert1.fingerprint, cert2.fingerprint);
}

#[test]
fn test_fingerprint_from_pem_deterministic() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let ca = cert_service::generate_ca_certificate().unwrap();
    let fp1 = cert_service::fingerprint_from_pem(&ca.certificate_pem).unwrap();
    let fp2 = cert_service::fingerprint_from_pem(&ca.certificate_pem).unwrap();
    assert_eq!(fp1, fp2);
}

#[test]
fn test_fingerprint_from_invalid_pem() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let result = cert_service::fingerprint_from_pem("not a valid PEM");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Unit tests: cert_repo
// ---------------------------------------------------------------------------

#[test]
fn test_ca_certificate_crud() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let pool = setup_test_db();
    let mut conn = pool.get().unwrap();

    // No CA initially
    let ca = cert_repo::get_ca_certificate(&mut conn).unwrap();
    assert!(ca.is_none());

    // Insert CA
    let new_ca = NewCaCertificate {
        private_key_pem: "test-key".to_string(),
        certificate_pem: "test-cert".to_string(),
    };
    let ca = cert_repo::insert_ca_certificate(&mut conn, &new_ca).unwrap();
    assert_eq!(ca.private_key_pem, "test-key");
    assert_eq!(ca.certificate_pem, "test-cert");

    // Retrieve CA
    let retrieved = cert_repo::get_ca_certificate(&mut conn).unwrap().unwrap();
    assert_eq!(retrieved.id, ca.id);
    assert_eq!(retrieved.certificate_pem, "test-cert");
}

#[test]
fn test_device_certificate_crud() {
    let _guard = TEST_DB_LOCK.blocking_lock();
    let pool = setup_test_db();
    let mut conn = pool.get().unwrap();

    // Generate CA and a device to reference
    let new_ca = cert_service::generate_ca_certificate().unwrap();
    insert_generated_ca(&mut conn, new_ca);
    drop(conn);

    // Create a device in the DB to satisfy the FK constraint
    use extrittio_backend::domains::devices::types::{CreateDeviceRecord, NewDeviceContractRecord};
    use extrittio_backend::persistence::postgres;
    use extrittio_backend::services::device_catalog_service;
    let ctx = test_context();
    let device = CreateDeviceRecord {
        id: "dev-cert-test".to_string(),
        name: "Cert Test Device".to_string(),
        device_type_id: 1,
        fleet_id: None,
        firmware: "v1".to_string(),
        contract: Some(NewDeviceContractRecord {
            id: "dev-cert-test-contract".to_string(),
            blueprint_revision_id: TEST_BLUEPRINT_REVISION_ID.to_string(),
            document: serde_json::json!({"contractApi": 1}),
            contract_hash: "0".repeat(64),
            created_at: chrono::Utc::now(),
        }),
    };
    let persistence = postgres::create_persistence(pool.clone());
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        device_catalog_service::create(
            &ctx,
            persistence.devices.as_ref(),
            persistence.certificates.as_ref(),
            device,
        )
        .await
        .unwrap();
    });
    let mut conn = pool.get().unwrap();

    // Device cert was auto-generated on device creation
    let cert = cert_repo::get_device_certificate(&mut conn, "dev-cert-test")
        .unwrap()
        .expect("Device cert should have been auto-generated");
    assert_eq!(cert.device_id, "dev-cert-test");
    assert!(!cert.private_key_pem.is_empty());
    assert!(
        cert.certificate_pem
            .starts_with("-----BEGIN CERTIFICATE-----")
    );

    // Clear private key
    cert_repo::clear_device_private_key(&mut conn, cert.id).unwrap();
    let after_clear = cert_repo::get_device_certificate(&mut conn, "dev-cert-test")
        .unwrap()
        .unwrap();
    assert!(after_clear.private_key_pem.is_empty());

    // Delete device certificates
    let deleted = cert_repo::delete_device_certificates(&mut conn, "dev-cert-test").unwrap();
    assert_eq!(deleted, 1);

    let gone = cert_repo::get_device_certificate(&mut conn, "dev-cert-test").unwrap();
    assert!(gone.is_none());
}

// ---------------------------------------------------------------------------
// Integration tests: certificate API endpoints
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_get_ca_certificate_api() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, _pool) = setup_app_with_ca().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/ca/certificate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let ca: Value = serde_json::from_slice(&body).unwrap();

    assert!(
        ca["certificate_pem"]
            .as_str()
            .unwrap()
            .starts_with("-----BEGIN CERTIFICATE-----")
    );
    assert!(ca["fingerprint"].as_str().unwrap().contains(':'));
    assert!(!ca["created_at"].as_str().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_certificate_lifecycle() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, _pool) = setup_app_with_ca().await;

    // Create a device (cert is auto-generated)
    let create_body = serde_json::json!({
        "name": "Cert Lifecycle Device",
        "device_type_id": 1,
        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/devices")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let device: Value = serde_json::from_slice(&body).unwrap();
    let device_id = device["id"].as_str().unwrap();

    // Check certificate status
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/certificate/status"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let status: Value = serde_json::from_slice(&body).unwrap();
    assert!(status["fingerprint"].as_str().unwrap().contains(':'));
    assert!(!status["expires_at"].as_str().unwrap().is_empty());

    // Download device certificate (first time — includes private key)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/certificate"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let cert: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        cert["certificate_pem"]
            .as_str()
            .unwrap()
            .starts_with("-----BEGIN CERTIFICATE-----")
    );
    assert!(
        cert["private_key_pem"]
            .as_str()
            .unwrap()
            .starts_with(concat!("-----BEGIN PRIVATE ", "KEY-----"))
    );
    assert!(
        cert["ca_pem"]
            .as_str()
            .unwrap()
            .starts_with("-----BEGIN CERTIFICATE-----")
    );

    // Second download should fail (private key cleared)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/certificate"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Regenerate the certificate
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/devices/{device_id}/certificate/regenerate"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let new_cert: Value = serde_json::from_slice(&body).unwrap();
    // New cert should have different fingerprint than original
    assert_ne!(
        new_cert["fingerprint"].as_str().unwrap(),
        cert["fingerprint"].as_str().unwrap()
    );
    // Should include private key again
    assert!(
        new_cert["private_key_pem"]
            .as_str()
            .unwrap()
            .starts_with(concat!("-----BEGIN PRIVATE ", "KEY-----"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_cert_for_nonexistent_device() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, _pool) = setup_app_with_ca().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices/nonexistent-id/certificate/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
