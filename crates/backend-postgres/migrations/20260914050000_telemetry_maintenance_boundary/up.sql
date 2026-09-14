CREATE TABLE telemetry_maintenance_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    pruned_before TIMESTAMPTZ
);
-- Legacy pruning was not tracked. Freeze pre-upgrade hours conservatively on
-- databases with telemetry history; a fresh database has no pruning boundary.
INSERT INTO telemetry_maintenance_state (singleton, pruned_before)
SELECT 1, CASE WHEN EXISTS (SELECT 1 FROM telemetry)
    OR EXISTS (SELECT 1 FROM telemetry_rollups_hourly)
    THEN now() ELSE NULL END;
