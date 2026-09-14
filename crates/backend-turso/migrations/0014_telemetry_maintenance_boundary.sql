CREATE TABLE telemetry_maintenance_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    pruned_before INTEGER
);
-- Historical raw completeness cannot be inferred after untracked pruning.
INSERT INTO telemetry_maintenance_state (singleton, pruned_before)
SELECT 1, CASE WHEN EXISTS (SELECT 1 FROM telemetry)
    OR EXISTS (SELECT 1 FROM telemetry_rollups_hourly)
    THEN CAST(unixepoch('subsec') * 1000000 AS INTEGER) ELSE NULL END;
