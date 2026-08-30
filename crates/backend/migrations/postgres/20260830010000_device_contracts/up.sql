CREATE TABLE device_contracts (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    blueprint_revision_id TEXT NOT NULL,
    document JSONB NOT NULL,
    contract_hash TEXT NOT NULL CHECK (length(contract_hash) = 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, device_id, contract_hash),
    FOREIGN KEY (tenant_id, device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, blueprint_revision_id)
        REFERENCES device_blueprint_revisions(tenant_id, id)
);

CREATE TABLE device_contract_assignments (
    tenant_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    desired_contract_id TEXT NOT NULL,
    active_contract_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'converged', 'failed')),
    acknowledged_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, device_id),
    FOREIGN KEY (tenant_id, device_id)
        REFERENCES devices(tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, desired_contract_id)
        REFERENCES device_contracts(tenant_id, id),
    FOREIGN KEY (tenant_id, active_contract_id)
        REFERENCES device_contracts(tenant_id, id)
);

CREATE INDEX idx_device_contracts_device_created
    ON device_contracts (tenant_id, device_id, created_at DESC);
CREATE INDEX idx_device_contract_assignments_status
    ON device_contract_assignments (tenant_id, status, updated_at);
