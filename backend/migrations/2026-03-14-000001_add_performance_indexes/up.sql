-- Performance indexes for frequently filtered columns not yet indexed

-- devices.status: used in list filtering, offline checker
CREATE INDEX idx_devices_status ON devices (status);

-- devices.last_seen: used by offline checker background task
CREATE INDEX idx_devices_last_seen ON devices (last_seen);
