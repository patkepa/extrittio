use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};

use crate::ApplicationError;
use crate::events::{DeviceEventRepository, MetricPruneOutcome, MetricRetentionCutoffs};

#[derive(Clone)]
pub struct MetricMaintenanceApplication {
    repository: Arc<dyn DeviceEventRepository>,
}

impl MetricMaintenanceApplication {
    pub fn new(repository: Arc<dyn DeviceEventRepository>) -> Self {
        Self { repository }
    }

    pub async fn prune(
        &self,
        raw_days: u64,
        rollup_days: u64,
        now: DateTime<Utc>,
    ) -> Result<MetricPruneOutcome, ApplicationError> {
        let cutoffs = retention_cutoffs(raw_days, rollup_days, now)?;
        Ok(self.repository.prune_metrics(cutoffs).await?)
    }
}

fn retention_cutoffs(
    raw_days: u64,
    rollup_days: u64,
    now: DateTime<Utc>,
) -> Result<MetricRetentionCutoffs, ApplicationError> {
    if raw_days == 0 || rollup_days < raw_days {
        return Err(ApplicationError::InvalidInput(
            "Metric rollup retention must be at least the positive raw retention period".into(),
        ));
    }
    let floor = |days: u64| -> Result<DateTime<Utc>, ApplicationError> {
        let days = i64::try_from(days)
            .map_err(|_| ApplicationError::InvalidInput("Metric retention is too large".into()))?;
        let age = TimeDelta::try_days(days).ok_or_else(|| {
            ApplicationError::InvalidInput("Metric retention is too large".into())
        })?;
        let cutoff = now.checked_sub_signed(age).ok_or_else(|| {
            ApplicationError::InvalidInput("Metric retention cutoff is out of range".into())
        })?;
        DateTime::from_timestamp(cutoff.timestamp().div_euclid(3_600) * 3_600, 0).ok_or_else(|| {
            ApplicationError::InvalidInput("Metric retention cutoff is out of range".into())
        })
    };
    Ok(MetricRetentionCutoffs {
        raw_retained_since: floor(raw_days)?,
        rollup_retained_since: floor(rollup_days)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_keeps_the_cutoff_hour_and_requires_rollups_to_outlive_raw_data() {
        let now = DateTime::from_timestamp(31 * 86_400 + 3_600 + 42, 0).unwrap();
        let cutoffs = retention_cutoffs(1, 30, now).unwrap();
        assert_eq!(cutoffs.raw_retained_since.timestamp(), 30 * 86_400 + 3_600);
        assert_eq!(cutoffs.rollup_retained_since.timestamp(), 86_400 + 3_600);
        assert!(retention_cutoffs(0, 30, now).is_err());
        assert!(retention_cutoffs(31, 30, now).is_err());
    }
}
