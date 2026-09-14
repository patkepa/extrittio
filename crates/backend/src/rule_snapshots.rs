//! Process-local immutable rule snapshots and bounded refresh scheduling.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use extrittio_backend_core::rule_engine::cache::RuleCache;
use extrittio_backend_core::rules::RuleRepository;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RuleSnapshotMetrics {
    pub age_seconds: f64,
    pub refresh_interval_seconds: u64,
    pub reload_duration_seconds: f64,
    pub reload_successes: u64,
    pub reload_failures: u64,
    pub definition_changes: u64,
    pub ready: bool,
}

struct ReloadState {
    last_success: Instant,
    fingerprint: String,
    duration: Duration,
    successes: u64,
    failures: u64,
    changes: u64,
}

pub struct RuleSnapshotStore {
    repository: Arc<dyn RuleRepository>,
    current: RwLock<Arc<RuleCache>>,
    state: Mutex<ReloadState>,
    single_flight: tokio::sync::Mutex<()>,
    invalidation: Notify,
    interval: Duration,
}

fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("rule snapshot state: {error}"))
}

fn fingerprint(
    records: &extrittio_backend_core::rule_snapshots::RuleSnapshotRecords,
) -> Result<String, AppError> {
    let bytes = records.canonical_definitions().map_err(internal)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

impl RuleSnapshotStore {
    /// Startup must finish this load before constructing a ready server.
    pub async fn initialize(
        repository: Arc<dyn RuleRepository>,
        interval: Duration,
    ) -> Result<Arc<Self>, AppError> {
        if !(1..=60).contains(&interval.as_secs()) || interval.subsec_nanos() != 0 {
            return Err(AppError::Internal(
                "rule snapshot interval must be 1–60 whole seconds".into(),
            ));
        }
        let started = Instant::now();
        let cache = repository.load_snapshot().await?;
        let fingerprint = fingerprint(&cache)?;
        Ok(Arc::new(Self {
            repository,
            current: RwLock::new(Arc::new(cache.compile())),
            state: Mutex::new(ReloadState {
                last_success: started,
                fingerprint,
                duration: started.elapsed(),
                successes: 1,
                failures: 0,
                changes: 1,
            }),
            single_flight: tokio::sync::Mutex::new(()),
            invalidation: Notify::new(),
            interval,
        }))
    }

    pub fn snapshot(
        &self,
    ) -> Result<extrittio_backend_core::rule_snapshots::RuleEvaluationSnapshot, AppError> {
        Ok(
            extrittio_backend_core::rule_snapshots::RuleEvaluationSnapshot::new(
                self.current.read().map_err(internal)?.clone(),
            ),
        )
    }

    /// Notify keeps one pending permit, including invalidations arriving during a reload.
    pub fn invalidate(&self) {
        self.invalidation.notify_one();
    }

    pub fn metrics(&self) -> Result<RuleSnapshotMetrics, AppError> {
        let state = self.state.lock().map_err(internal)?;
        let age = state.last_success.elapsed();
        Ok(RuleSnapshotMetrics {
            age_seconds: age.as_secs_f64(),
            refresh_interval_seconds: self.interval.as_secs(),
            reload_duration_seconds: state.duration.as_secs_f64(),
            reload_successes: state.successes,
            reload_failures: state.failures,
            definition_changes: state.changes,
            ready: age <= self.interval * 2,
        })
    }

    pub async fn reload(&self) -> Result<(), AppError> {
        let Ok(_flight) = self.single_flight.try_lock() else {
            self.invalidate();
            return Ok(());
        };
        let started = Instant::now();
        let loaded = async {
            let cache = self.repository.load_snapshot().await?;
            let fingerprint = fingerprint(&cache)?;
            Ok::<_, AppError>((cache, fingerprint))
        }
        .await;
        let mut state = self.state.lock().map_err(internal)?;
        state.duration = started.elapsed();
        match loaded {
            Ok((records, fingerprint)) => {
                if fingerprint != state.fingerprint {
                    let mut cache = records.compile();
                    let mut current = self.current.write().map_err(internal)?;
                    cache.version = current.version.wrapping_add(1);
                    *current = Arc::new(cache);
                    state.fingerprint = fingerprint;
                    state.changes += 1;
                }
                state.duration = started.elapsed();
                state.last_success = started;
                state.successes += 1;
                Ok(())
            }
            Err(error) => {
                state.failures += 1;
                Err(error)
            }
        }
    }

    pub async fn run(&self) -> Result<(), String> {
        let mut ticks =
            tokio::time::interval_at(tokio::time::Instant::now() + self.interval, self.interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ticks.tick() => {},
                _ = self.invalidation.notified() => {},
            }
            if let Err(error) = self.reload().await {
                tracing::warn!(%error, "Rule snapshot reload failed; retaining last good definitions");
            }
        }
    }
}

impl extrittio_backend_core::rules::RuleChangeNotifier for RuleSnapshotStore {
    fn committed(&self) {
        self.invalidate();
    }
}

impl extrittio_backend_core::rule_snapshots::RuleSnapshotProvider for RuleSnapshotStore {
    fn snapshot(
        &self,
    ) -> Result<
        extrittio_backend_core::rule_snapshots::RuleEvaluationSnapshot,
        extrittio_backend_core::ApplicationError,
    > {
        RuleSnapshotStore::snapshot(self).map_err(|error| {
            extrittio_backend_core::ApplicationError::Internal(format!(
                "failed to obtain rule snapshot: {error}"
            ))
        })
    }
}
