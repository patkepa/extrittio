use super::require_permission;
use crate::telemetry::{
    TelemetryMaintenanceOutcome, TelemetryQuery, TelemetryRecord, TelemetryRepository,
    TelemetryRollup,
};
use crate::{ApplicationError, Clock, Permission, TenantContext};
use chrono::Timelike;
use std::sync::Arc;
#[derive(Clone)]
pub struct TelemetryApplication {
    repository: Arc<dyn TelemetryRepository>,
}
impl TelemetryApplication {
    pub fn new(repository: Arc<dyn TelemetryRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        mut query: TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        query.limit = query.limit.clamp(1, 1000);
        self.repository
            .list(ctx.tenant_id(), device_id, query)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }

    pub async fn latest(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        Ok(self.repository.latest(ctx.tenant_id(), device_id).await?)
    }

    pub async fn list_hourly(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        mut query: TelemetryQuery,
    ) -> Result<Vec<TelemetryRollup>, ApplicationError> {
        require_permission(ctx, Permission::ReadTelemetry)?;
        query.limit = query.limit.clamp(1, 10_000);
        self.repository
            .list_hourly(ctx.tenant_id(), device_id, query)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Device '{device_id}' not found")))
    }

    /// Preserve the existing authenticated, tenant-scoped location read behavior.
    /// Unlike raw/history reads, this legacy endpoint had no telemetry permission gate.
    pub async fn latest_location(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, ApplicationError> {
        Ok(self
            .repository
            .latest_location(ctx.tenant_id(), device_id)
            .await?)
    }
}
#[derive(Clone)]
pub struct TelemetryMaintenanceApplication {
    repository: Arc<dyn TelemetryRepository>,
    clock: Arc<dyn Clock>,
}
impl TelemetryMaintenanceApplication {
    pub fn new(repository: Arc<dyn TelemetryRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repository, clock }
    }
    pub async fn maintain(
        &self,
        retention_days: u64,
    ) -> Result<TelemetryMaintenanceOutcome, ApplicationError> {
        let now = self.clock.now().naive_utc();
        let range_error = || {
            ApplicationError::InvalidInput(
                "Telemetry maintenance time range is outside the supported range".into(),
            )
        };
        let current_hour = now
            .with_minute(0)
            .and_then(|value| value.with_second(0))
            .and_then(|value| value.with_nanosecond(0))
            .ok_or_else(range_error)?;
        let since = current_hour
            .checked_sub_signed(chrono::Duration::hours(25))
            .ok_or_else(range_error)?;
        let cutoff = i64::try_from(retention_days)
            .ok()
            .and_then(chrono::Duration::try_days)
            .and_then(|days| now.checked_sub_signed(days))
            .ok_or_else(range_error)?;
        Ok(self
            .repository
            .maintain(since, current_hour, cutoff)
            .await?)
    }
}

/// Owns raw telemetry compatibility, targeting retries, and atomic write intent.
#[derive(Clone)]
pub struct TelemetryIngressApplication {
    repository: Arc<dyn TelemetryRepository>,
    ingress: Arc<dyn crate::device_ingress::DeviceIngressRepository>,
    clock: Arc<dyn Clock>,
}
impl TelemetryIngressApplication {
    pub fn new(
        repository: Arc<dyn TelemetryRepository>,
        ingress: Arc<dyn crate::device_ingress::DeviceIngressRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repository,
            ingress,
            clock,
        }
    }

    pub async fn record(
        &self,
        identity: &crate::DeviceIdentity,
        input: crate::telemetry::TelemetryInput,
        rules: &dyn crate::rule_snapshots::RuleSnapshotProvider,
    ) -> Result<crate::telemetry::TelemetryWriteOutcome, ApplicationError> {
        use crate::rule_snapshots::{DeviceRuleEvaluation, RuleEvaluationInput};
        use crate::telemetry::{TelemetryWrite, TelemetryWriteOutcome};
        // Receipt precedes context lookup and stays fixed across retries. Normalize
        // once so PostgreSQL and Turso persist the same microsecond instant.
        let received = self.clock.now();
        let received_at = received
            .with_nanosecond(received.nanosecond() / 1_000 * 1_000)
            .expect("microsecond truncation is a valid nanosecond")
            .naive_utc();
        // Older clients omit the presence flag; newer clients can report (0, 0).
        let has_location = input.has_location || input.latitude != 0.0 || input.longitude != 0.0;
        let custom_json = if input.metadata.is_empty() {
            None
        } else {
            serde_json::to_value(&input.metadata).ok()
        };
        let data = crate::rule_engine::types::TelemetryData {
            temperature: input.temperature,
            humidity: input.humidity,
            battery_level: input.battery_level,
            latitude: has_location.then_some(input.latitude),
            longitude: has_location.then_some(input.longitude),
            speed: input.speed,
            altitude: input.altitude,
            heading: input.heading,
            metrics: std::collections::BTreeMap::new(),
        };
        for _ in 0..3 {
            let Some(context) = self.ingress.ingress_context(identity).await? else {
                return Ok(TelemetryWriteOutcome {
                    recorded: false,
                    actions_enqueued: 0,
                });
            };
            let observed_at = self.clock.now().naive_utc();
            let rule_evaluation = DeviceRuleEvaluation {
                snapshot: rules.snapshot()?,
                tenant: identity.tenant_id().clone(),
                device_id: identity.device_id().to_owned(),
                device_type_id: context.device_type_id,
                fleet_id: context.fleet_id,
                blueprint_id: context.blueprint_id,
                input: RuleEvaluationInput::Telemetry {
                    data: data.clone(),
                    geofence: true,
                },
                observed_at,
            };
            let outcome = self
                .repository
                .record(
                    identity,
                    TelemetryWrite {
                        expected_device_type_id: context.device_type_id,
                        expected_fleet_id: context.fleet_id,
                        payload: input.payload.clone(),
                        temperature: Some(input.temperature),
                        humidity: Some(input.humidity),
                        battery_level: Some(input.battery_level),
                        custom_json: custom_json.clone(),
                        latitude: data.latitude,
                        longitude: data.longitude,
                        speed: has_location.then_some(input.speed),
                        altitude: has_location.then_some(input.altitude),
                        heading: has_location.then_some(input.heading),
                        rule_evaluation,
                        received_at,
                        observed_at,
                    },
                )
                .await?;
            if outcome.recorded {
                return Ok(outcome);
            }
        }
        Err(ApplicationError::Persistence(
            crate::PersistenceError::Busy { retry_after: None },
        ))
    }
}
