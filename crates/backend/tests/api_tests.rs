use axum::body::Body;
use axum::http::{Request, StatusCode};
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use http_body_util::BodyExt;
use prost::Message;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

use extrittio_backend::api_key_util;
use extrittio_backend::rate_limit::{ApiKeyRateLimiter, RateLimiter};
use extrittio_backend::state::AppState;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
const DEFAULT_TEST_DATABASE_URL: &str =
    "postgres://extrittio:extrittio@127.0.0.1:5432/extrittio?connect_timeout=2";
static TEST_DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn tenant_context(tenant_id: &str) -> extrittio_backend::auth::context::RequestContext {
    extrittio_backend::auth::context::RequestContext::from_claims(extrittio_backend::auth::Claims {
        sub: 1,
        username: "admin".to_string(),
        role: "admin".to_string(),
        tenant_id: Some(tenant_id.to_string()),
        scopes: Vec::new(),
        permission_version: 1,
        exp: 0,
    })
}

fn test_context() -> extrittio_backend::auth::context::RequestContext {
    tenant_context(extrittio_backend::tenancy::DEFAULT_TENANT_ID)
}

fn scoped_context(scopes: &[&str]) -> extrittio_backend::auth::context::RequestContext {
    extrittio_backend::auth::context::RequestContext::from_claims(extrittio_backend::auth::Claims {
        sub: 2,
        username: "operator".to_string(),
        role: "viewer".to_string(),
        tenant_id: Some(extrittio_backend::tenancy::DEFAULT_TENANT_ID.to_string()),
        scopes: scopes.iter().map(|scope| (*scope).to_string()).collect(),
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
    diesel::sql_query("TRUNCATE TABLE rule_action_outbox, rule_cooldowns, alerts, rule_actions, rule_conditions, rules, zones, app_metrics, server_metrics, device_certificates, ca_certificates, command_history, device_logs, ota_deployments, firmware_blobs, firmware_updates, api_keys, device_configs, device_shadows, telemetry_rollups_hourly, telemetry, devices, fleets, device_types, users, server_config CASCADE")
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

    pool
}

async fn setup_app_with_context(
    ctx: extrittio_backend::auth::context::RequestContext,
) -> (axum::Router, Pool<ConnectionManager<PgConnection>>) {
    let db_pool = setup_test_db();
    let zenoh_session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open test zenoh session");

    let state = Arc::new(extrittio_backend::state::AppState {
        db_pool: db_pool.clone(),
        zenoh_session: Arc::new(zenoh_session),
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
    });

    let app = extrittio_backend::api::router(100 * 1024 * 1024, true)
        .layer(axum::Extension(ctx))
        .with_state(state);

    (app, db_pool)
}

async fn setup_app() -> axum::Router {
    setup_app_with_context(test_context()).await.0
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_list_devices_empty() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["total"], 0);
    assert!(result["data"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_policy_rejects_missing_permission() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app_with_context(scoped_context(&["devices.read"]))
        .await
        .0;

    let create_body = serde_json::json!({
        "name": "Blocked Device",
        "device_type_id": 1
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_create_user_defaults_to_viewer_role() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let create_body = serde_json::json!({
        "username": "viewer-user",
        "password": "Secret123!456"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/users")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["username"], "viewer-user");
    assert_eq!(created["role"], "viewer");
    assert_eq!(created["roles"][0]["name"], "viewer");
    assert!(
        created["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "devices.read")
    );
    assert!(
        !created["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "users.manage")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_devices_are_scoped_to_request_tenant() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, pool) = setup_app_with_context(test_context()).await;

    {
        let mut conn = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO organizations (id, name) VALUES ('tenant-b', 'Tenant B') ON CONFLICT DO NOTHING",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_types (id, tenant_id, name) VALUES (3, 'tenant-b', 'default') ON CONFLICT DO NOTHING",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO devices (id, tenant_id, name, device_type_id, firmware) VALUES \
             ('default-device', 'default', 'Default Tenant Device', 1, 'v1'), \
             ('tenant-b-device', 'tenant-b', 'Tenant B Device', 3, 'v1')",
        )
        .execute(&mut conn)
        .unwrap();
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["total"], 1);
    let devices = result["data"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], "default-device");

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices/tenant-b-device")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_database_rejects_cross_tenant_device_relationships() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (_app, pool) = setup_app_with_context(test_context()).await;
    let mut conn = pool.get().unwrap();

    diesel::sql_query(
        "INSERT INTO organizations (id, name) VALUES ('tenant-b', 'Tenant B') ON CONFLICT DO NOTHING",
    )
    .execute(&mut conn)
    .unwrap();

    let result = diesel::sql_query(
        "INSERT INTO devices (id, tenant_id, name, device_type_id, firmware) \
         VALUES ('cross-tenant-device', 'tenant-b', 'Invalid Device', 1, 'v1')",
    )
    .execute(&mut conn);

    assert!(
        result.is_err(),
        "a tenant must not reference another tenant's device type"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_ingress_resolves_the_persisted_tenant_identity() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (_app, pool) = setup_app_with_context(test_context()).await;

    {
        let mut conn = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO organizations (id, name) VALUES ('tenant-b', 'Tenant B') ON CONFLICT DO NOTHING",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_types (id, tenant_id, name) VALUES (3, 'tenant-b', 'default') ON CONFLICT DO NOTHING",
        )
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO devices (id, tenant_id, name, device_type_id, firmware) \
             VALUES ('tenant-b-ingress', 'tenant-b', 'Tenant B Ingress', 3, 'v1')",
        )
        .execute(&mut conn)
        .unwrap();
    }

    let telemetry = extrittio_common::extrittio::DeviceTelemetry {
        device_id: "tenant-b-ingress".to_string(),
        timestamp: 1,
        temperature: 21.5,
        humidity: 48.0,
        battery_level: 90.0,
        metadata: std::collections::HashMap::new(),
        latitude: 0.0,
        longitude: 0.0,
        speed: 0.0,
        altitude: 0.0,
        heading: 0.0,
    };
    let rule_cache =
        std::sync::RwLock::new(extrittio_backend::rule_engine::cache::RuleCache::default());
    extrittio_backend::zenoh_handler::handlers::telemetry::handle_telemetry(
        &pool,
        "tenant-b-ingress",
        &telemetry.encode_to_vec(),
        &rule_cache,
    );

    let mut conn = pool.get().unwrap();
    let tenant_id = extrittio_backend::db::schema::telemetry::table
        .filter(extrittio_backend::db::schema::telemetry::device_id.eq("tenant-b-ingress"))
        .select(extrittio_backend::db::schema::telemetry::tenant_id)
        .first::<String>(&mut conn)
        .unwrap();
    assert_eq!(tenant_id, "tenant-b");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_create_and_get_device() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    // POST to create a device (device_type_id=1 is the seeded "default")
    let create_body = serde_json::json!({
        "name": "Sensor A",
        "device_type_id": 1,
        "firmware": "v1.0.0"
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
    let created: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["name"], "Sensor A");
    assert_eq!(created["device_type_id"], 1);
    assert_eq!(created["device_type_name"], "default");
    assert_eq!(created["firmware"], "v1.0.0");
    assert_eq!(created["status"], "offline");
    assert!(created["fleet_id"].is_null());
    assert!(created["fleet_name"].is_null());

    let device_id = created["id"].as_str().unwrap();

    // GET the created device
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{}", device_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let fetched: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(fetched["id"], device_id);
    assert_eq!(fetched["name"], "Sensor A");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_update_device() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    // Create a device first
    let create_body = serde_json::json!({
        "name": "Sensor B",
        "device_type_id": 1
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
    let created: Value = serde_json::from_slice(&body).unwrap();
    let device_id = created["id"].as_str().unwrap().to_string();

    // PUT to update the device partially
    let update_body = serde_json::json!({
        "name": "Sensor B Updated"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/devices/{}", device_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let updated: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "Sensor B Updated");
    // device_type should remain unchanged
    assert_eq!(updated["device_type_name"], "default");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_delete_device() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    // Create a device
    let create_body = serde_json::json!({
        "name": "Sensor C",
        "device_type_id": 1
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
    let created: Value = serde_json::from_slice(&body).unwrap();
    let device_id = created["id"].as_str().unwrap().to_string();

    // DELETE the device
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/devices/{}", device_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // GET should return 404
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{}", device_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_get_nonexistent_device() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices/nonexistent-id-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_dashboard_stats() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    // Create two devices
    for name in &["Device X", "Device Y"] {
        let create_body = serde_json::json!({
            "name": name,
            "device_type_id": 1
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
    }

    // GET dashboard stats
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/dashboard/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let stats: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(stats["total_devices"], 2);
    // Both devices default to "offline" status
    assert_eq!(stats["active_devices"], 0);
    assert_eq!(stats["offline_devices"], 2);
    assert_eq!(stats["total_messages"], 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_filter_devices_by_status() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    // Create a device (defaults to "offline")
    let create_body = serde_json::json!({
        "name": "Filter Test Device",
        "device_type_id": 1
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

    // Filter by status=online — should be empty
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices?status=online")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["total"], 0);
    assert!(result["data"].as_array().unwrap().is_empty());

    // Filter by status=offline — should return 1 result
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/devices?status=offline")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["total"], 1);
    let devices = result["data"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["name"], "Filter Test Device");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_ci_ingest_unauthorized() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/firmware-updates/ci")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"device_type":"default","version":"1.0.0","artifact_url":"https://example.com/fw.bin"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_ci_ingest_invalid_key() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/firmware-updates/ci")
                .header("Content-Type", "application/json")
                .header("Authorization", "Bearer extr_invalidkey")
                .body(Body::from(
                    r#"{"device_type":"default","version":"1.0.0","artifact_url":"https://example.com/fw.bin","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_ci_ingest_success() {
    let _guard = TEST_DB_LOCK.lock().await;
    // Setup with shared db_pool so we can insert API key directly
    let db_pool = setup_test_db();
    let zenoh_session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open test zenoh session");

    let state = Arc::new(AppState {
        db_pool: db_pool.clone(),
        zenoh_session: Arc::new(zenoh_session),
        jwt_secret: "test-secret-key".to_string(),
        public_url: "http://localhost:8080".to_string(),
        cookie_secure: false,
        health_token: None,
        api_rate_limiter: RateLimiter::new(10000, 60),
        login_rate_limiter: RateLimiter::new(10000, 60),
        trusted_proxies: extrittio_backend::rate_limit::parse_trusted_proxies(&[]),
        ci_rate_limiter: ApiKeyRateLimiter::new(10000, 60),
        metrics_accumulator: extrittio_backend::state::MetricsAccumulator::new(),
        zenoh_metrics: Arc::new(extrittio_backend::state::ZenohMetrics::new()),
        rule_cache: Arc::new(std::sync::RwLock::new(
            extrittio_backend::rule_engine::cache::RuleCache::default(),
        )),
        http_client: reqwest::Client::new(),
        firmware_store: extrittio_backend::domains::firmware_store::FirmwareObjectStore::in_memory(
        ),
        readiness: Arc::new(extrittio_backend::state::ReadinessRegistry::new(true, true)),
    });

    let app = extrittio_backend::api::router(100 * 1024 * 1024, true).with_state(state);

    // Insert a test API key directly into the DB
    let test_key = "test-api-key";
    let key_hash = api_key_util::hash_api_key(test_key);
    let key_prefix = api_key_util::key_prefix(test_key);

    {
        let mut conn = db_pool.get().unwrap();
        diesel::sql_query(format!(
            "INSERT INTO api_keys (name, key_hash, key_prefix, device_type_id) VALUES ('Test Key', '{}', '{}', NULL)",
            key_hash, key_prefix
        ))
        .execute(&mut conn)
        .unwrap();
    }

    // Call CI ingest endpoint with the valid key
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/firmware-updates/ci")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", test_key))
                .body(Body::from(
                    r#"{"device_type":"default","version":"1.0.0-ci-test","artifact_url":"https://example.com/fw.bin","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","commit_sha":"abc123","branch":"main"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["version"], "1.0.0-ci-test");
    assert_eq!(result["device_type"], "default");
}
