use crate::device_ingress::{
    DeviceIngressContext, DeviceIngressRepository, HeartbeatWrite, OfflineTransition,
    OfflineWriteOutcome,
};
use crate::rule_engine::types::StatusChange;
use crate::rule_snapshots::RuleSnapshotProvider;
use crate::{ApplicationError, Clock, DeviceIdentity};
use std::sync::Arc;
const MAX_OPTIMISTIC_RETRIES: usize = 3;

fn prepare_evaluation(
    context: &DeviceIngressContext,
    new_status: &str,
    cache: &dyn RuleSnapshotProvider,
    observed_at: chrono::NaiveDateTime,
) -> Result<Option<crate::rule_snapshots::DeviceRuleEvaluation>, ApplicationError> {
    if context.status == new_status {
        return Ok(None);
    }
    Ok(Some(crate::rule_snapshots::DeviceRuleEvaluation {
        snapshot: cache.snapshot().map_err(|e| {
            ApplicationError::Internal(format!("failed to obtain rule snapshot: {e}"))
        })?,
        tenant: context.identity.tenant_id().clone(),
        device_id: context.identity.device_id().to_owned(),
        fleet_id: context.fleet_id,
        blueprint_id: context.blueprint_id.clone(),
        input: crate::rule_snapshots::RuleEvaluationInput::Status(StatusChange {
            old_status: context.status.clone(),
            new_status: new_status.to_owned(),
        }),
        observed_at,
    }))
}

#[derive(Clone)]
pub struct DeviceIngressApplication {
    repository: Arc<dyn DeviceIngressRepository>,
    clock: Arc<dyn Clock>,
}
impl DeviceIngressApplication {
    pub fn new(repository: Arc<dyn DeviceIngressRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn apply_heartbeat(
        &self,
        identity: DeviceIdentity,
        reported_status: &str,
        firmware: &str,
        uptime_seconds: i64,
        rule_cache: &dyn RuleSnapshotProvider,
    ) -> Result<usize, ApplicationError> {
        let status = if ["online", "offline", "warning"]
            .iter()
            .any(|status| status.eq_ignore_ascii_case(reported_status))
        {
            reported_status.to_string()
        } else {
            "online".to_string()
        };

        for _ in 0..MAX_OPTIMISTIC_RETRIES {
            let Some(context) = self.repository.ingress_context(&identity).await? else {
                return Ok(0);
            };
            let observed_at = self.clock.now().naive_utc();
            let rule_evaluation = prepare_evaluation(&context, &status, rule_cache, observed_at)?;
            #[allow(clippy::cast_possible_truncation)]
            let outcome = self
                .repository
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
        Err(ApplicationError::Persistence(
            crate::PersistenceError::Busy { retry_after: None },
        ))
    }

    pub async fn mark_offline_devices(
        &self,
        timeout_secs: u64,
        rule_cache: &dyn RuleSnapshotProvider,
    ) -> Result<OfflineWriteOutcome, ApplicationError> {
        let now = self.clock.now().naive_utc();
        let cutoff = i64::try_from(timeout_secs)
            .ok()
            .and_then(chrono::TimeDelta::try_seconds)
            .and_then(|duration| now.checked_sub_signed(duration))
            .ok_or_else(|| {
                ApplicationError::InvalidInput(
                    "Offline timeout is outside the supported range".into(),
                )
            })?;
        let candidates = self.repository.offline_candidates(cutoff).await?;
        let mut transitions = Vec::with_capacity(candidates.len());
        let observed_at = self.clock.now().naive_utc();
        for context in candidates {
            let rule_evaluation = prepare_evaluation(&context, "offline", rule_cache, observed_at)?;
            transitions.push(OfflineTransition {
                context,
                rule_evaluation,
            });
        }
        Ok(self
            .repository
            .apply_offline_transitions(cutoff, transitions)
            .await?)
    }

    /// Resolves the globally unique protocol identifier before tenant-owned ingress.
    pub async fn resolve_identity(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceIdentity>, ApplicationError> {
        if !crate::device_identity::valid_device_id(device_id) {
            return Err(ApplicationError::InvalidInput("Device ID must be 1-128 characters and contain only letters, numbers, '-', '_', '.', or ':'".into()));
        }
        Ok(self.repository.resolve_identity(device_id).await?)
    }
}
