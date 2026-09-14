use chrono::Utc;

use crate::error::AppError;
use crate::rule_engine::types::StatusChange;
use crate::tenancy::DeviceIdentity;

use crate::domains::devices::repository::DeviceIngressRepository;

use crate::domains::devices::types::{
    DeviceIngressContext, HeartbeatWrite, OfflineTransition, OfflineWriteOutcome,
};

const MAX_OPTIMISTIC_RETRIES: usize = 3;

fn prepare_evaluation(
    context: &DeviceIngressContext,
    new_status: &str,
    cache: &crate::rule_snapshots::RuleSnapshotStore,
    observed_at: chrono::NaiveDateTime,
) -> Result<Option<extrittio_backend_core::rule_snapshots::DeviceRuleEvaluation>, AppError> {
    if context.status == new_status {
        return Ok(None);
    }
    Ok(Some(
        extrittio_backend_core::rule_snapshots::DeviceRuleEvaluation {
            snapshot: cache
                .snapshot()
                .map_err(|e| AppError::Internal(format!("failed to obtain rule snapshot: {e}")))?,
            tenant: context.identity.tenant_id().clone(),
            device_id: context.identity.device_id().to_owned(),
            device_type_id: context.device_type_id,
            fleet_id: context.fleet_id,
            blueprint_id: context.blueprint_id.clone(),
            input: extrittio_backend_core::rule_snapshots::RuleEvaluationInput::Status(
                StatusChange {
                    old_status: context.status.clone(),
                    new_status: new_status.to_owned(),
                },
            ),
            observed_at,
        },
    ))
}

pub async fn apply_heartbeat(
    repository: &dyn DeviceIngressRepository,
    identity: DeviceIdentity,
    reported_status: &str,
    firmware: &str,
    uptime_seconds: i64,
    rule_cache: &crate::rule_snapshots::RuleSnapshotStore,
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
        let observed_at = Utc::now().naive_utc();
        let rule_evaluation = prepare_evaluation(&context, &status, rule_cache, observed_at)?;
        #[allow(clippy::cast_possible_truncation)]
        let outcome = repository
            .apply_heartbeat(
                &identity,
                HeartbeatWrite {
                    expected_status: context.status,
                    status: status.clone(),
                    firmware: firmware.to_string(),
                    uptime_seconds: uptime_seconds as i32,
                    observed_at,
                    rule_evaluation,
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
    repository: &dyn DeviceIngressRepository,
    cutoff: chrono::NaiveDateTime,
    rule_cache: &crate::rule_snapshots::RuleSnapshotStore,
) -> Result<OfflineWriteOutcome, AppError> {
    let candidates = repository.offline_candidates(cutoff).await?;
    let mut transitions = Vec::with_capacity(candidates.len());
    let observed_at = Utc::now().naive_utc();
    for context in candidates {
        let rule_evaluation = prepare_evaluation(&context, "offline", rule_cache, observed_at)?;
        transitions.push(OfflineTransition {
            context,
            rule_evaluation,
        });
    }
    Ok(repository
        .apply_offline_transitions(cutoff, transitions)
        .await?)
}

/// Resolves the globally unique protocol identifier before tenant-owned ingress.
pub async fn resolve_identity(
    repository: &dyn DeviceIngressRepository,
    device_id: &str,
) -> Result<Option<DeviceIdentity>, AppError> {
    if !extrittio_common::topics::is_valid_device_id(device_id) {
        return Err(AppError::BadRequest("Device ID must be 1-128 characters and contain only letters, numbers, '-', '_', '.', or ':'".into()));
    }
    Ok(repository.resolve_identity(device_id).await?)
}
