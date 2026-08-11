use std::sync::RwLock;

use chrono::Utc;

use crate::error::AppError;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::evaluate::evaluate_status_change_for_tenant;
use crate::rule_engine::types::StatusChange;
use crate::tenancy::{DEFAULT_TENANT_ID, DeviceIdentity, TenantId};

use crate::domains::devices::repository::DeviceRepository;
use crate::domains::devices::types::{
    AutoRegisterOutcome, DeviceIngressContext, HeartbeatWrite, OfflineTransition,
    OfflineWriteOutcome,
};

const MAX_OPTIMISTIC_RETRIES: usize = 3;

fn infer_device_type(firmware: &str) -> &'static str {
    let firmware = firmware.to_ascii_lowercase();
    if firmware.contains("network-analyzer") || firmware.contains("network_analyzer") {
        "network-analyzer"
    } else if firmware.contains("organbath")
        || firmware.contains("organ-bath")
        || firmware.contains("organ_bath")
    {
        "OrganBath"
    } else if firmware.contains("macos") {
        "mac-device"
    } else {
        "default"
    }
}

fn evaluated_actions(
    context: &DeviceIngressContext,
    new_status: &str,
    cache: &RwLock<RuleCache>,
) -> Result<Vec<crate::rule_engine::types::PendingAction>, AppError> {
    if context.status == new_status {
        return Ok(Vec::new());
    }
    let change = StatusChange {
        old_status: context.status.clone(),
        new_status: new_status.to_string(),
    };
    let cache = cache
        .read()
        .map_err(|error| AppError::Internal(format!("failed to read-lock rule cache: {error}")))?;
    Ok(evaluate_status_change_for_tenant(
        context.identity.tenant_id_str(),
        context.identity.device_id(),
        context.device_type_id,
        context.fleet_id,
        &change,
        &cache,
    ))
}

pub async fn auto_register(
    repository: &dyn DeviceRepository,
    device_id: &str,
    firmware: &str,
) -> Result<Option<DeviceIngressContext>, AppError> {
    if !extrittio_common::topics::is_valid_device_id(device_id) {
        return Ok(None);
    }
    let tenant = TenantId::new(DEFAULT_TENANT_ID)
        .map_err(|error| AppError::Internal(format!("invalid default tenant: {error}")))?;
    match repository
        .auto_register(&tenant, device_id, firmware, infer_device_type(firmware))
        .await?
    {
        AutoRegisterOutcome::Existing(context) | AutoRegisterOutcome::Created(context) => {
            Ok(Some(context))
        }
        AutoRegisterOutcome::NoDeviceType => Ok(None),
    }
}

pub async fn apply_heartbeat(
    repository: &dyn DeviceRepository,
    identity: DeviceIdentity,
    reported_status: &str,
    firmware: &str,
    uptime_seconds: i64,
    rule_cache: &RwLock<RuleCache>,
) -> Result<usize, AppError> {
    let status = if extrittio_common::device_status::is_valid(reported_status) {
        reported_status.to_string()
    } else {
        extrittio_common::device_status::ONLINE.to_string()
    };

    for _ in 0..MAX_OPTIMISTIC_RETRIES {
        let Some(context) = repository.ingress_context(&identity).await? else {
            return Ok(0);
        };
        let pending_actions = evaluated_actions(&context, &status, rule_cache)?;
        #[allow(clippy::cast_possible_truncation)]
        let outcome = repository
            .apply_heartbeat(
                &identity,
                HeartbeatWrite {
                    expected_status: context.status,
                    status: status.clone(),
                    firmware: firmware.to_string(),
                    uptime_seconds: uptime_seconds as i32,
                    observed_at: Utc::now().naive_utc(),
                    pending_actions,
                },
            )
            .await?;
        if outcome.applied {
            return Ok(outcome.actions_enqueued);
        }
    }
    Err(AppError::Persistence(
        crate::persistence::PersistenceError::Busy { retry_after: None },
    ))
}

pub async fn mark_offline_devices(
    repository: &dyn DeviceRepository,
    cutoff: chrono::NaiveDateTime,
    rule_cache: &RwLock<RuleCache>,
) -> Result<OfflineWriteOutcome, AppError> {
    let candidates = repository.offline_candidates(cutoff).await?;
    let mut transitions = Vec::with_capacity(candidates.len());
    for context in candidates {
        let pending_actions = evaluated_actions(&context, "offline", rule_cache)?;
        transitions.push(OfflineTransition {
            context,
            pending_actions,
        });
    }
    Ok(repository
        .apply_offline_transitions(cutoff, transitions)
        .await?)
}
