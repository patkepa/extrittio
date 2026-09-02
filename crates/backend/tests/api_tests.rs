#![cfg(feature = "postgres")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use http_body_util::BodyExt;
use prost::Message;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

use extrittio_backend::api_key_util;
use extrittio_backend::rate_limit::{ApiKeyRateLimiter, RateLimiter};
use extrittio_backend::state::{AppState, AppStateInput};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../backend-postgres/migrations");
const DEFAULT_TEST_DATABASE_URL: &str =
    "postgres://extrittio:extrittio@127.0.0.1:5432/extrittio?connect_timeout=2";
const TEST_BLUEPRINT_ID: &str = "test-default-blueprint";
const TEST_BLUEPRINT_REVISION_ID: &str = "test-default-blueprint-r1";
static TEST_DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize)]
struct LegacyClaimsWithoutAuthEpoch<'a> {
    sub: i32,
    username: &'a str,
    role: &'a str,
    tenant_id: &'a str,
    scopes: Vec<String>,
    permission_version: i32,
    exp: usize,
}

#[derive(QueryableByName)]
struct AuthEpochRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    auth_epoch: String,
}

fn tenant_context(tenant_id: &str) -> extrittio_backend::auth::context::RequestContext {
    extrittio_backend::auth::context::RequestContext::from_claims(extrittio_backend::auth::Claims {
        sub: 1,
        username: "admin".to_string(),
        role: "admin".to_string(),
        tenant_id: Some(tenant_id.to_string()),
        scopes: Vec::new(),
        permission_version: 1,
        auth_epoch: Some("test-auth-epoch".to_string()),
        exp: 0,
    })
    .expect("test claims contain a valid tenant")
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
        auth_epoch: Some("test-auth-epoch".to_string()),
        exp: 0,
    })
    .expect("test claims contain a valid tenant")
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
    diesel::sql_query("TRUNCATE TABLE device_metric_samples, device_events, device_contract_assignments, device_contracts, device_blueprint_revisions, device_blueprint_drafts, device_blueprints, rule_action_outbox, rule_cooldowns, alerts, rule_actions, rule_conditions, rules, zones, app_metrics, server_metrics, device_certificates, ca_certificates, command_history, device_logs, ota_deployments, firmware_blobs, firmware_updates, api_keys, device_configs, device_shadows, telemetry_rollups_hourly, telemetry, devices, fleets, device_types, users, server_config CASCADE")
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
    diesel::sql_query(
        "INSERT INTO device_blueprints (id, tenant_id, blueprint_key, name)
         VALUES ($1, 'default', 'test-default', 'Test default')",
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
    .bind::<diesel::sql_types::Text, _>(
        blueprint_document("test-default", "Test default").to_string(),
    )
    .bind::<diesel::sql_types::Text, _>("0".repeat(64))
    .execute(&mut conn)
    .unwrap();

    pool
}

async fn setup_app_with_context_and_state(
    ctx: extrittio_backend::auth::context::RequestContext,
) -> (
    axum::Router,
    Pool<ConnectionManager<PgConnection>>,
    Arc<AppState>,
) {
    extrittio_backend::init::install_crypto_provider();
    let db_pool = setup_test_db();
    let zenoh_session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open test zenoh session");
    let database = extrittio_backend::persistence::postgres::create_runtime(db_pool.clone());

    let state = Arc::new(AppState::new(AppStateInput {
        database,
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
    }));

    let app = extrittio_backend::api::router(100 * 1024 * 1024, true)
        .layer(axum::Extension(ctx))
        .with_state(state.clone());

    (app, db_pool, state)
}

async fn setup_app_with_context(
    ctx: extrittio_backend::auth::context::RequestContext,
) -> (axum::Router, Pool<ConnectionManager<PgConnection>>) {
    let (app, pool, _state) = setup_app_with_context_and_state(ctx).await;
    (app, pool)
}

async fn setup_app() -> axum::Router {
    setup_app_with_context(test_context()).await.0
}

fn blueprint_document(key: &str, name: &str) -> Value {
    serde_json::json!({
        "apiVersion": "extrittio.io/v1alpha1",
        "kind": "DeviceBlueprint",
        "metadata": {"key": key, "name": name},
        "spec": {
            "runtime": {
                "minimumContractApi": 1,
                "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
            },
            "firmware": {
                "strategy": "binary_replacement",
                "compatibility": {"board": "test-board"}
            }
        }
    })
}

fn contract_blueprint_document() -> Value {
    serde_json::json!({
        "apiVersion": "extrittio.io/v1alpha1",
        "kind": "DeviceBlueprint",
        "metadata": {"key": "freezer", "name": "Freezer Monitor"},
        "spec": {
            "runtime": {
                "minimumContractApi": 1,
                "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
            },
            "transports": [{
                "key": "primary", "binding": "site_bus", "protocol": "zenoh"
            }],
            "schemas": [{
                "key": "reading@1", "format": "json_schema",
                "schema": {
                    "type": "object",
                    "properties": {"temperature": {"type": "number"}},
                    "required": ["temperature"],
                    "additionalProperties": false
                }
            }],
            "routes": [{
                "key": "readings", "transport": "primary", "direction": "device_to_cloud",
                "address": "extrittio/devices/{device.id}/events/readings",
                "messageSchema": "reading@1", "encoding": "json"
            }, {
                "key": "command_requests", "transport": "primary", "direction": "cloud_to_device",
                "address": "extrittio/devices/{device.id}/commands/request",
                "messageSchema": "extrittio.command-request@1", "encoding": "protobuf"
            }, {
                "key": "command_results", "transport": "primary", "direction": "device_to_cloud",
                "address": "extrittio/devices/{device.id}/commands/result",
                "messageSchema": "extrittio.command-result@1", "encoding": "protobuf"
            }],
            "streams": [{
                "key": "environment", "route": "readings",
                "fields": [{
                    "path": "/temperature", "type": "float64", "label": "Temperature",
                    "unit": "Cel", "index": true, "aggregates": ["min", "max", "avg"]
                }]
            }],
            "commands": [{
                "key": "set_limit", "requestRoute": "command_requests",
                "responseRoute": "command_results", "label": "Set alarm limit", "danger": "confirm",
                "timeout": "10s", "idempotency": "idempotent",
                "inputSchema": {
                    "type": "object",
                    "properties": {"temperature": {"type": "number", "minimum": -40, "maximum": 40}},
                    "required": ["temperature"], "additionalProperties": false
                },
                "resultSchema": {
                    "type": "object",
                    "properties": {"applied": {"type": "boolean"}},
                    "required": ["applied"], "additionalProperties": false
                }
            }],
            "configuration": {
                "schema": {
                    "type": "object",
                    "properties": {
                        "sampleSeconds": {"type": "integer", "minimum": 5, "default": 30}
                    },
                    "required": ["sampleSeconds"],
                    "additionalProperties": false
                },
                "defaults": {"sampleSeconds": 30},
                "apply": {
                    "mode": "desired_reported", "acknowledgementTimeout": "30s", "atomic": true
                }
            }
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_role_http_contract_and_permission_catalog_order_are_preserved() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, pool) = setup_app_with_context(test_context()).await;
    diesel::delete(
        extrittio_backend::db::schema::roles::table
            .filter(extrittio_backend::db::schema::roles::is_system.eq(false)),
    )
    .execute(&mut pool.get().unwrap())
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/roles/permissions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let permissions: Value = serde_json::from_slice(&body).unwrap();
    let expected = [
        "firmware.deploy",
        "alerts.manage",
        "device_blueprints.manage",
        "device_types.manage",
        "devices.manage",
        "api_keys.manage",
        "firmware.manage",
        "fleets.manage",
        "rules.manage",
        "roles.manage",
        "shadows.manage",
        "users.manage",
        "zones.manage",
        "commands.read",
        "alerts.read",
        "device_blueprints.read",
        "device_types.read",
        "devices.read",
        "fleets.read",
        "firmware.read",
        "logs.read",
        "rules.read",
        "roles.read",
        "server_metrics.read",
        "shadows.read",
        "telemetry.read",
        "users.read",
        "zones.read",
        "commands.send",
    ];
    assert_eq!(
        permissions,
        Value::Array(
            expected
                .iter()
                .map(|key| serde_json::json!({ "key": key }))
                .collect()
        )
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/roles")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"owner","description":null,"permissions":[]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "bad_request");
    assert_eq!(error["message"], "'owner' is reserved for a built-in role");
    assert_eq!(error["error"], error["message"]);

    let create = || {
        Request::builder()
            .method("POST")
            .uri("/api/v1/roles")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"name":"Support_Team","description":"Escalations","permissions":["devices.read","devices.read"]}"#,
            ))
            .unwrap()
    };
    let response = app.clone().oneshot(create()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let role: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(role["name"], "support_team");
    assert_eq!(role["description"], "Escalations");
    assert_eq!(role["is_system"], false);
    assert_eq!(role["permissions"], serde_json::json!(["devices.read"]));
    assert_eq!(role["user_count"], 0);

    let response = app.oneshot(create()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "conflict");
    assert_eq!(error["message"], "Role name already exists");
    assert_eq!(error["error"], error["message"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_blueprint_draft_validation_and_publication() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/device-blueprints")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "document": blueprint_document("cold-room", "Cold Room")
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let created: Value = serde_json::from_slice(&body).unwrap();
    let blueprint_id = created["id"].as_str().unwrap();
    assert_eq!(created["key"], "cold-room");
    assert!(created["latest_revision"].is_null());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/device-blueprints/{blueprint_id}/draft/validate"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let validation: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["issues"], serde_json::json!([]));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/device-blueprints/{blueprint_id}/draft/publish"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let revision: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(revision["revision"], 1);
    assert_eq!(revision["document_hash"].as_str().unwrap().len(), 64);
    assert_eq!(revision["compatibility"]["breaking"], false);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/device-blueprints/{blueprint_id}/revisions/latest"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let latest: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(latest["id"], revision["id"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_creation_materializes_and_assigns_contract() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, pool) = setup_app_with_context(test_context()).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/device-blueprints")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "document": contract_blueprint_document()
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let blueprint: Value = serde_json::from_slice(&body).unwrap();
    let blueprint_id = blueprint["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/device-blueprints/{}/draft/publish",
                    blueprint_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let revision: Value = serde_json::from_slice(&body).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/devices")
                .header("content-type", "application/json")
                .header("host", "hub.example.test:8080")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "name": "Freezer A",
                        "device_type_id": 1,
                        "blueprint_revision_id": revision["id"],
                        "configuration": {"sampleSeconds": 10}
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let device: Value = serde_json::from_slice(&body).unwrap();
    let device_id = device["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/contract"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let contract: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(contract["assignment_status"], "pending");
    assert_eq!(contract["contract_hash"].as_str().unwrap().len(), 64);
    assert_eq!(
        contract["document"]["transports"]["primary"]["endpoint"],
        "tcp/hub.example.test:7447"
    );
    assert_eq!(
        contract["document"]["routes"]["readings"]["address"],
        format!("extrittio/devices/{device_id}/events/readings")
    );
    assert_eq!(
        contract["document"]["configuration"]["desired"]["sampleSeconds"],
        10
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/devices/{device_id}/commands"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "command": "set_limit",
                        "params": {"temperature": "not-a-number"}
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let persistence = extrittio_backend::persistence::postgres::create_repositories(pool.clone());
    let identity = extrittio_backend::tenancy::DeviceIdentity::new(
        extrittio_backend::tenancy::DEFAULT_TENANT_ID,
        device_id,
    )
    .unwrap();
    let envelope = serde_json::json!({
        "apiVersion": 1,
        "eventId": "2b7e1516-28ae-4f2b-a6ab-f7158809cf4f",
        "contractHash": contract["contract_hash"],
        "occurredAt": chrono::Utc::now().to_rfc3339(),
        "payload": {"temperature": 21.5}
    });
    let metrics_recorded = extrittio_backend::zenoh_handler::handlers::event::handle_event(
        &persistence,
        &identity,
        "readings",
        &serde_json::to_vec(&envelope).unwrap(),
        &std::sync::RwLock::new(extrittio_backend::rule_engine::cache::RuleCache::default()),
    )
    .await;
    assert_eq!(metrics_recorded, 1);

    let assigned = persistence
        .devices
        .assigned_contract(identity.tenant_id(), device_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(assigned.assignment_status, "converged");
    assert_eq!(assigned.contract_hash, contract["contract_hash"]);
    let ingress = persistence
        .devices
        .ingress_context(&identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ingress.blueprint_id.as_deref(), Some(blueprint_id));
    let mut connection = pool.get().unwrap();
    let metric_count = extrittio_backend::db::schema::device_metric_samples::table
        .filter(extrittio_backend::db::schema::device_metric_samples::device_id.eq(device_id))
        .count()
        .get_result::<i64>(&mut connection)
        .unwrap();
    assert_eq!(metric_count, 1);
    drop(connection);
    let stored_metrics = persistence
        .events
        .list_metrics(
            identity.tenant_id(),
            device_id,
            extrittio_backend::domains::events::types::DeviceMetricQuery {
                stream_key: Some("environment".into()),
                field_path: Some("/temperature".into()),
                since: None,
                before: None,
                limit: 10,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_metrics.len(), 1);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/devices/{device_id}/metrics?stream_key=environment&field_path=%2Ftemperature"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let metrics: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(metrics.as_array().unwrap().len(), 1);
    assert_eq!(metrics[0]["stream_key"], "environment");
    assert_eq!(metrics[0]["field_path"], "/temperature");
    assert_eq!(metrics[0]["value_type"], "float64");
    assert_eq!(metrics[0]["value"], 21.5);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/analytics/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let catalog: Value = serde_json::from_slice(&body).unwrap();
    assert!(catalog["metrics"].as_array().unwrap().iter().any(|metric| {
        metric["blueprint_id"] == blueprint_id
            && metric["stream_key"] == "environment"
            && metric["field_path"] == "/temperature"
            && metric["unit"] == "Cel"
    }));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/analytics/query")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "scope": {"device_ids": [device_id]},
                        "metric": {
                            "blueprint_id": blueprint_id,
                            "stream_key": "environment",
                            "field_path": "/temperature"
                        },
                        "from": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
                        "to": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
                        "mode": "per_device"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let analytics: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(analytics["metric"]["key"], "environment./temperature");
    assert_eq!(analytics["metric"]["blueprint_id"], blueprint_id);
    assert_eq!(analytics["scope"]["compatible_devices"], 1);
    assert_eq!(analytics["series"][0]["stats"]["average"], 21.5);
    assert_eq!(analytics["effective"]["source"], "blueprint_metric_samples");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/analytics/query")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "scope": {"device_ids": [device_id]},
                        "metric": {
                            "blueprint_id": blueprint_id,
                            "stream_key": "environment",
                            "field_path": "/undeclared"
                        },
                        "from": (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
                        "to": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_creation_requires_a_blueprint_revision() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/devices")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Contractless","device_type_id":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_firmware_is_registered_against_a_blueprint_revision() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/firmware-updates")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID,
                        "version": "2.0.0",
                        "url": "https://firmware.example.com/test-device-2.0.0.bin",
                        "sha256": "a".repeat(64)
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let firmware: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        firmware["blueprint_revision_id"],
        TEST_BLUEPRINT_REVISION_ID
    );
    assert_eq!(firmware["update_strategy"], "binary_replacement");
    assert_eq!(firmware["compatibility"]["board"], "test-board");

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/firmware-updates?blueprint_revision_id={TEST_BLUEPRINT_REVISION_ID}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let listed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed["total"], 1);
    assert_eq!(
        listed["data"][0]["blueprint_revision_id"],
        TEST_BLUEPRINT_REVISION_ID
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_ready_checks_persistence_health() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
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
async fn test_fleet_lifecycle_uses_tenant_scoped_persistence() {
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
            "INSERT INTO fleets (tenant_id, name) VALUES ('tenant-b', 'Hidden Fleet')",
        )
        .execute(&mut conn)
        .unwrap();
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/fleets")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "name": "Production" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let fleet_id = created["id"].as_i64().unwrap();
    assert_eq!(created["name"], "Production");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/fleets/{fleet_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "name": "Renamed" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/fleets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let listed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["data"][0]["id"], fleet_id);
    assert_eq!(listed["data"][0]["name"], "Renamed");
    assert_eq!(listed["data"][0]["device_count"], 0);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/fleets/{fleet_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/fleets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let listed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed["total"], 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_type_lifecycle_uses_tenant_scoped_persistence() {
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
            "INSERT INTO device_types (tenant_id, name, icon, color_hex) VALUES ('tenant-b', 'Hidden Type', 'cube', '#8ABBFF')",
        )
        .execute(&mut conn)
        .unwrap();
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/device-types")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "name": "  Air Sensor  ",
                        "icon": "air-quality",
                        "color_hex": "#aabbcc"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let device_type_id = created["id"].as_i64().unwrap();
    assert_eq!(created["name"], "Air Sensor");
    assert_eq!(created["color_hex"], "#AABBCC");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/device-types/{device_type_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "name": "Environment" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let updated: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "Environment");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/device-types")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let listed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed["total"], 3);
    assert!(
        listed["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["id"] == device_type_id && record["name"] == "Environment")
    );
    assert!(
        listed["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|record| record["name"] != "Hidden Type")
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/device-types/{device_type_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_configuration_merges_atomically() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/devices")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "name": "Config Device",
                        "device_type_id": 1,
                        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let device_id = created["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/config"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let empty: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(empty["config"], serde_json::json!({}));

    for patch in [
        serde_json::json!({ "enabled": true, "interval": 10 }),
        serde_json::json!({ "enabled": null, "interval": 20 }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/devices/{device_id}/config"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&patch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/config"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let config: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(config["config"], serde_json::json!({ "interval": 20 }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_device_shadow_mutations_return_the_committed_state() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/devices")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "name": "Shadow Device",
                        "device_type_id": 1,
                        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let device_id = created["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/devices/{device_id}/shadow/desired"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"sample_rate":10}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let desired: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(desired["desired"], serde_json::json!({ "sample_rate": 10 }));
    assert_eq!(desired["delta"], serde_json::json!({ "sample_rate": 10 }));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/devices/{device_id}/shadow/reported"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"sample_rate":10}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let reported: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        reported["reported"],
        serde_json::json!({ "sample_rate": 10 })
    );
    assert_eq!(reported["delta"], serde_json::json!({}));
    assert_eq!(
        reported["version"],
        desired["version"].as_i64().unwrap() + 1
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/devices/{device_id}/shadow"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/devices/{device_id}/shadow"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let reset: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(reset["desired"], serde_json::json!({}));
    assert_eq!(reset["reported"], serde_json::json!({}));
    assert_eq!(reset["delta"], serde_json::json!({}));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_policy_rejects_missing_permission() {
    let _guard = TEST_DB_LOCK.lock().await;
    let app = setup_app_with_context(scoped_context(&["devices.read"]))
        .await
        .0;

    let create_body = serde_json::json!({
        "name": "Blocked Device",
        "device_type_id": 1,
        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID
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
async fn test_login_cookie_token_me_and_session_invalidation() {
    let _guard = TEST_DB_LOCK.lock().await;
    let (app, pool, state) = setup_app_with_context_and_state(test_context()).await;
    let password = "Secret123!456";

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "username": "login-user",
                        "password": password,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: Value = serde_json::from_slice(
        &create_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let user_id = i32::try_from(created["id"].as_i64().unwrap()).unwrap();

    let login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "username": "login-user",
                        "password": password,
                        "issue_token": true,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    let set_cookie = login_response
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.starts_with("extrittio_session="));
    assert!(set_cookie.contains("; HttpOnly; SameSite=Strict; Max-Age=86400"));
    assert!(!set_cookie.contains("; Secure"));
    let login: Value = serde_json::from_slice(
        &login_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let token = login["token"].as_str().unwrap().to_string();
    assert_eq!(login["user"]["id"], user_id);
    assert_eq!(login["user"]["username"], "login-user");
    assert_eq!(login["user"]["role"], "viewer");
    assert_eq!(login["user"]["permission_version"], 2);

    let claims = extrittio_backend::auth::validate_token(&token, "test-secret-key").unwrap();
    assert_eq!(claims.sub, user_id);
    let persisted_tenant = claims
        .tenant_id
        .as_deref()
        .expect("new sessions carry an explicit tenant");
    assert_eq!(persisted_tenant, "default");
    assert_eq!(claims.permission_version, 2);
    let issued_auth_epoch = claims
        .auth_epoch
        .as_deref()
        .filter(|value| !value.is_empty())
        .expect("new sessions carry a nonempty authentication epoch")
        .to_string();
    assert!(login["user"].get("auth_epoch").is_none());

    let authenticated_app = extrittio_backend::api::router(100 * 1024 * 1024, true)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            extrittio_backend::middleware::auth_middleware,
        ))
        .with_state(state);
    let me_with_bearer = authenticated_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_with_bearer.status(), StatusCode::OK);
    let current: Value = serde_json::from_slice(
        &me_with_bearer
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(current, login["user"]);

    let cookie_pair = set_cookie.split(';').next().unwrap();
    let me_with_cookie = authenticated_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("cookie", cookie_pair)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_with_cookie.status(), StatusCode::OK);

    let legacy_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &LegacyClaimsWithoutAuthEpoch {
            sub: user_id,
            username: "login-user",
            role: "viewer",
            tenant_id: persisted_tenant,
            scopes: claims.scopes.clone(),
            permission_version: 2,
            exp: usize::try_from((chrono::Utc::now() + chrono::Duration::hours(1)).timestamp())
                .unwrap(),
        },
        &jsonwebtoken::EncodingKey::from_secret(b"test-secret-key"),
    )
    .unwrap();
    let legacy_rejected = authenticated_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {legacy_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_rejected.status(), StatusCode::UNAUTHORIZED);

    diesel::sql_query(
        "UPDATE users SET permission_version = permission_version + 1 \
         WHERE tenant_id = $1 AND id = $2",
    )
    .bind::<diesel::sql_types::Text, _>(persisted_tenant)
    .bind::<diesel::sql_types::Integer, _>(user_id)
    .execute(&mut pool.get().unwrap())
    .unwrap();

    let invalidated = authenticated_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalidated.status(), StatusCode::UNAUTHORIZED);

    let replacement_password_hash = extrittio_backend::auth::hash_password(password).unwrap();
    let mut connection = pool.get().unwrap();
    diesel::sql_query("DELETE FROM users WHERE tenant_id = $1 AND id = $2")
        .bind::<diesel::sql_types::Text, _>(persisted_tenant)
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .execute(&mut connection)
        .unwrap();
    let replacement = diesel::sql_query(
        "INSERT INTO users \
             (id, tenant_id, username, password_hash, role, is_active, permission_version) \
         VALUES ($2, $1, 'login-user', $3, 'viewer', TRUE, 2) \
         RETURNING auth_epoch",
    )
    .bind::<diesel::sql_types::Text, _>(persisted_tenant)
    .bind::<diesel::sql_types::Integer, _>(user_id)
    .bind::<diesel::sql_types::Text, _>(replacement_password_hash)
    .get_result::<AuthEpochRow>(&mut connection)
    .unwrap();
    assert_ne!(replacement.auth_epoch, issued_auth_epoch);
    diesel::sql_query(
        "INSERT INTO user_roles (user_id, role_id, tenant_id) \
         SELECT $1, id, $2 FROM roles WHERE tenant_id = $2 AND name = 'viewer'",
    )
    .bind::<diesel::sql_types::Integer, _>(user_id)
    .bind::<diesel::sql_types::Text, _>(persisted_tenant)
    .execute(&mut connection)
    .unwrap();
    drop(connection);

    let replacement_cannot_revive_deleted_session = authenticated_app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        replacement_cannot_revive_deleted_session.status(),
        StatusCode::UNAUTHORIZED
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
        has_location: false,
    };
    let rule_cache =
        std::sync::RwLock::new(extrittio_backend::rule_engine::cache::RuleCache::default());
    let persistence = extrittio_backend::persistence::postgres::create_repositories(pool.clone());
    let identity = persistence
        .devices
        .resolve_identity("tenant-b-ingress")
        .await
        .unwrap()
        .unwrap();
    extrittio_backend::zenoh_handler::handlers::telemetry::handle_telemetry(
        &persistence,
        &identity,
        "tenant-b-ingress",
        &telemetry.encode_to_vec(),
        &rule_cache,
    )
    .await;

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

    // Blueprint-based creation does not expose the legacy storage type.
    let create_body = serde_json::json!({
        "name": "Sensor A",
        "blueprint_revision_id": TEST_BLUEPRINT_REVISION_ID,
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
    let database = extrittio_backend::persistence::postgres::create_runtime(db_pool.clone());

    let state = Arc::new(AppState::new(AppStateInput {
        database,
        zenoh_session: Arc::new(zenoh_session),
        zenoh_tls_enabled: false,
        zenoh_port: 7447,
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
        thread_runtime: None,
    }));

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
