// Command service — business logic for device commands

use prost::Message;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{CommandRecord, NewCommandRecord};
use crate::error::AppError;
use crate::repositories::{command_repo, device_repo};
use crate::state::{DbPool, ZenohMetrics, run_db};
use crate::tenancy::{DEFAULT_TENANT_ID, DeviceIdentity};
use extrittio_common::extrittio::DeviceCommand;

pub fn authorize_send_commands(ctx: &RequestContext) -> Result<(), AppError> {
    policy::require(ctx, Permission::SendCommands)
}

/// Send a command to a device: verify it exists, persist the record, publish via
/// Zenoh, then return the persisted record (with DB-generated timestamps).
///
/// DB operations run on blocking threads via `run_db` so the Tokio runtime is
/// not starved by synchronous Diesel calls.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the device does not exist, or other
/// `AppError` variants on database/Zenoh failures.
#[allow(clippy::implicit_hasher)]
pub async fn send_command_as_user(
    ctx: &RequestContext,
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<CommandRecord, AppError> {
    policy::require(ctx, Permission::SendCommands)?;
    send_command_for_tenant(
        ctx.tenant_id_str(),
        pool,
        zenoh_session,
        device_id,
        command,
        params,
        zenoh_metrics,
    )
    .await
}

#[allow(clippy::implicit_hasher)]
pub async fn send_command(
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<CommandRecord, AppError> {
    send_command_for_tenant(
        DEFAULT_TENANT_ID,
        pool,
        zenoh_session,
        device_id,
        command,
        params,
        zenoh_metrics,
    )
    .await
}

#[allow(clippy::implicit_hasher)]
async fn send_command_for_tenant(
    tenant_id: &str,
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<CommandRecord, AppError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());
    let d_id = device_id.to_string();
    let cmd = command.to_string();
    let corr_id = correlation_id.clone();
    let tenant_id = tenant_id.to_string();
    let tenant_id_for_insert = tenant_id.clone();

    // Verify device exists and persist the command record
    run_db(pool, move |conn| {
        device_repo::find_device_for_tenant(conn, &tenant_id_for_insert, &d_id)?;
        let new_record = NewCommandRecord {
            id: corr_id,
            tenant_id: tenant_id_for_insert,
            device_id: d_id,
            command: cmd,
            params: params_json,
        };
        command_repo::insert_command(conn, &new_record)?;
        Ok(())
    })
    .await?;

    // Build and publish protobuf (async, outside spawn_blocking)
    let proto_command = DeviceCommand {
        command: command.to_string(),
        params,
        correlation_id: correlation_id.clone(),
    };

    let payload = proto_command.encode_to_vec();
    let topic = extrittio_common::topics::commands(device_id);

    zenoh_session
        .put(&topic, payload)
        .await
        .map_err(|e| AppError::Zenoh(e.to_string()))?;

    zenoh_metrics.messages_out.fetch_add(1, Ordering::Relaxed);

    // Re-read the record to get the DB-generated timestamps
    let corr_id = correlation_id;
    run_db(pool, move |conn| {
        Ok(command_repo::find_command_for_tenant(
            conn, &tenant_id, &corr_id,
        )?)
    })
    .await
}

/// Terminal command statuses — commands in these states should not be updated.
const TERMINAL_STATUSES: &[&str] = &["succeeded", "failed", "timed_out"];

/// Handle a command response from a device (used by Zenoh handler).
pub fn handle_response(
    conn: &mut PgConnection,
    identity: &DeviceIdentity,
    correlation_id: &str,
    device_status: &str,
    payload: Option<&str>,
) -> Result<Option<String>, AppError> {
    if correlation_id.is_empty() {
        return Ok(None);
    }

    let command =
        command_repo::find_command_for_tenant(conn, identity.tenant_id_str(), correlation_id)?;
    if command.device_id != identity.device_id() {
        return Ok(None);
    }

    if TERMINAL_STATUSES.contains(&command.status.as_str()) {
        return Ok(None);
    }

    let new_status = match device_status {
        "ack" => "delivered",
        "succeeded" | "failed" => device_status,
        _ => "delivered",
    };

    let now = chrono::Utc::now().naive_utc();
    command_repo::update_command_status(
        conn,
        identity.tenant_id_str(),
        correlation_id,
        new_status,
        payload,
        now,
    )?;

    Ok(Some(new_status.to_string()))
}

/// List commands for a device with optional status filter.
/// Returns 404 if the device does not exist.
pub fn list_commands(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<CommandRecord>, AppError> {
    policy::require(ctx, Permission::ReadCommands)?;

    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    Ok(command_repo::list_commands(
        conn,
        ctx.tenant_id_str(),
        device_id,
        status,
        limit,
    )?)
}

/// Mark stale commands as timed out.
pub fn timeout_stale(conn: &mut PgConnection, timeout_secs: u64) -> Result<usize, AppError> {
    #[allow(clippy::cast_possible_wrap)]
    let cutoff = chrono::Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);
    let now = chrono::Utc::now().naive_utc();
    Ok(command_repo::timeout_stale_commands(conn, cutoff, now)?)
}
