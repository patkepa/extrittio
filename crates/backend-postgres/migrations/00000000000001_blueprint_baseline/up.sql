-- Blueprint-only schema for new installations. No legacy upgrade path.

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

CREATE TABLE public.alerts (
    id text NOT NULL,
    rule_id text,
    device_id text NOT NULL,
    severity text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    message text NOT NULL,
    triggered_value text,
    resolved_at timestamp with time zone,
    acknowledged_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    CONSTRAINT alerts_severity_check CHECK ((severity = ANY (ARRAY['info'::text, 'warning'::text, 'critical'::text]))),
    CONSTRAINT alerts_status_check CHECK ((status = ANY (ARRAY['active'::text, 'acknowledged'::text, 'resolved'::text])))
);

CREATE TABLE public.api_keys (
    id integer NOT NULL,
    name text NOT NULL,
    key_hash text NOT NULL,
    key_prefix text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    blueprint_id text
);

CREATE SEQUENCE public.api_keys_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.api_keys_id_seq OWNED BY public.api_keys.id;

CREATE TABLE public.app_metrics (
    id bigint NOT NULL,
    request_count integer NOT NULL,
    error_count integer NOT NULL,
    avg_latency_ms real NOT NULL,
    p95_latency_ms real NOT NULL,
    db_pool_active integer NOT NULL,
    db_pool_idle integer NOT NULL,
    zenoh_messages_in integer NOT NULL,
    zenoh_messages_out integer NOT NULL,
    recorded_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE SEQUENCE public.app_metrics_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.app_metrics_id_seq OWNED BY public.app_metrics.id;

CREATE TABLE public.audit_events (
    id text NOT NULL,
    tenant_id text NOT NULL,
    actor_type text NOT NULL,
    actor_id text,
    action text NOT NULL,
    resource_type text NOT NULL,
    resource_id text,
    outcome text NOT NULL,
    request_id text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.ca_certificates (
    id integer NOT NULL,
    private_key_pem text NOT NULL,
    certificate_pem text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE SEQUENCE public.ca_certificates_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.ca_certificates_id_seq OWNED BY public.ca_certificates.id;

CREATE TABLE public.command_history (
    id text NOT NULL,
    device_id text NOT NULL,
    command text NOT NULL,
    params text DEFAULT '{}'::text NOT NULL,
    status text DEFAULT 'sent'::text NOT NULL,
    response_payload text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE TABLE public.device_blueprint_drafts (
    id text NOT NULL,
    tenant_id text NOT NULL,
    blueprint_id text NOT NULL,
    document jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.device_blueprint_revisions (
    id text NOT NULL,
    tenant_id text NOT NULL,
    blueprint_id text NOT NULL,
    revision integer NOT NULL,
    document jsonb NOT NULL,
    document_hash text NOT NULL,
    compatibility jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT device_blueprint_revisions_document_hash_check CHECK ((length(document_hash) = 64)),
    CONSTRAINT device_blueprint_revisions_revision_check CHECK ((revision > 0))
);

CREATE TABLE public.device_blueprints (
    id text NOT NULL,
    tenant_id text NOT NULL,
    blueprint_key text NOT NULL,
    name text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.device_certificates (
    id integer NOT NULL,
    device_id text NOT NULL,
    private_key_pem text NOT NULL,
    certificate_pem text NOT NULL,
    fingerprint text NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE SEQUENCE public.device_certificates_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.device_certificates_id_seq OWNED BY public.device_certificates.id;

CREATE TABLE public.device_configs (
    device_id text NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE TABLE public.device_contract_assignments (
    tenant_id text NOT NULL,
    device_id text NOT NULL,
    desired_contract_id text NOT NULL,
    active_contract_id text,
    status text DEFAULT 'pending'::text NOT NULL,
    acknowledged_at timestamp with time zone,
    error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT device_contract_assignments_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'converged'::text, 'failed'::text])))
);

CREATE TABLE public.device_contracts (
    id text NOT NULL,
    tenant_id text NOT NULL,
    device_id text NOT NULL,
    blueprint_revision_id text NOT NULL,
    document jsonb NOT NULL,
    contract_hash text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT device_contracts_contract_hash_check CHECK ((length(contract_hash) = 64))
);

CREATE TABLE public.device_events (
    id text NOT NULL,
    tenant_id text NOT NULL,
    device_id text NOT NULL,
    contract_id text NOT NULL,
    route_key text NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    payload jsonb NOT NULL
);

CREATE TABLE public.device_logs (
    id bigint NOT NULL,
    device_id text NOT NULL,
    level text DEFAULT 'INFO'::text NOT NULL,
    message text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE SEQUENCE public.device_logs_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.device_logs_id_seq OWNED BY public.device_logs.id;

CREATE TABLE public.device_metric_samples (
    event_id text NOT NULL,
    tenant_id text NOT NULL,
    device_id text NOT NULL,
    stream_key text NOT NULL,
    field_path text NOT NULL,
    value_type text NOT NULL,
    value_double double precision,
    value_int bigint,
    value_text text,
    value_bool boolean,
    value_json jsonb,
    occurred_at timestamp with time zone NOT NULL,
    CONSTRAINT device_metric_samples_value_type_check CHECK ((value_type = ANY (ARRAY['float64'::text, 'int64'::text, 'string'::text, 'boolean'::text, 'json'::text])))
);

CREATE TABLE public.device_shadows (
    device_id text NOT NULL,
    desired jsonb DEFAULT '{}'::jsonb NOT NULL,
    reported jsonb DEFAULT '{}'::jsonb NOT NULL,
    delta jsonb DEFAULT '{}'::jsonb NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE TABLE public.devices (
    id text NOT NULL,
    name text NOT NULL,
    fleet_id integer,
    status text DEFAULT 'offline'::text NOT NULL,
    firmware text DEFAULT ''::text NOT NULL,
    last_seen timestamp with time zone,
    uptime_seconds integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    declared_connections jsonb DEFAULT '[]'::jsonb NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE TABLE public.firmware_blobs (
    firmware_update_id integer NOT NULL,
    size integer NOT NULL,
    filename text NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    storage_key text NOT NULL,
    storage_backend text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT firmware_blobs_size_check CHECK ((size >= 0)),
    CONSTRAINT firmware_blobs_storage_backend_check CHECK ((storage_backend = ANY (ARRAY['local'::text, 's3'::text])))
);

CREATE TABLE public.firmware_updates (
    id integer NOT NULL,
    version text NOT NULL,
    url text NOT NULL,
    description text,
    sha256 text,
    commit_sha text,
    branch text,
    ci_run_url text,
    build_timestamp timestamp with time zone,
    changelog text,
    source text DEFAULT 'manual'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    blueprint_revision_id text NOT NULL,
    compatibility jsonb DEFAULT '{}'::jsonb NOT NULL,
    update_strategy text
);

CREATE SEQUENCE public.firmware_updates_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.firmware_updates_id_seq OWNED BY public.firmware_updates.id;

CREATE TABLE public.fleets (
    id integer NOT NULL,
    name text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE SEQUENCE public.fleets_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.fleets_id_seq OWNED BY public.fleets.id;

CREATE TABLE public.organizations (
    id text NOT NULL,
    name text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.ota_deployments (
    id integer NOT NULL,
    device_id text NOT NULL,
    firmware_update_id integer NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    error_message text,
    initiated_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE SEQUENCE public.ota_deployments_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.ota_deployments_id_seq OWNED BY public.ota_deployments.id;

CREATE TABLE public.role_permissions (
    role_id integer NOT NULL,
    permission text NOT NULL
);

CREATE TABLE public.roles (
    id integer NOT NULL,
    tenant_id text NOT NULL,
    name text NOT NULL,
    description text,
    is_system boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE SEQUENCE public.roles_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.roles_id_seq OWNED BY public.roles.id;

CREATE TABLE public.rule_action_outbox (
    id text NOT NULL,
    tenant_id text NOT NULL,
    event_type text NOT NULL,
    aggregate_type text NOT NULL,
    aggregate_id text NOT NULL,
    payload jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 10 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    locked_at timestamp with time zone,
    locked_by text,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    idempotency_key text,
    CONSTRAINT rule_action_outbox_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'processing'::text, 'succeeded'::text, 'failed'::text, 'dead_letter'::text])))
);

CREATE TABLE public.rule_actions (
    id text NOT NULL,
    rule_id text NOT NULL,
    action_type text NOT NULL,
    config jsonb NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    CONSTRAINT rule_actions_action_type_check CHECK ((action_type = ANY (ARRAY['alert'::text, 'webhook'::text, 'command'::text])))
);

CREATE TABLE public.rule_alert_deliveries (
    delivery_id text NOT NULL,
    tenant_id text NOT NULL,
    alert_id text NOT NULL
);

CREATE TABLE public.rule_conditions (
    id text NOT NULL,
    rule_id text NOT NULL,
    field text NOT NULL,
    operator text NOT NULL,
    value text NOT NULL,
    condition_group integer DEFAULT 0 NOT NULL,
    zone_id text,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    CONSTRAINT rule_conditions_operator_check CHECK ((operator = ANY (ARRAY['gt'::text, 'gte'::text, 'lt'::text, 'lte'::text, 'eq'::text, 'neq'::text])))
);

CREATE TABLE public.rule_cooldowns (
    rule_id text NOT NULL,
    device_id text NOT NULL,
    last_fired_at timestamp with time zone NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

CREATE TABLE public.rule_zone_entries (
    tenant_id text NOT NULL,
    rule_id text NOT NULL,
    device_id text NOT NULL,
    entered_at timestamp with time zone NOT NULL
);

CREATE TABLE public.rules (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    enabled boolean DEFAULT true NOT NULL,
    trigger_type text NOT NULL,
    target_type text NOT NULL,
    target_id text,
    cooldown_seconds integer DEFAULT 300 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    CONSTRAINT rules_target_type_check CHECK ((target_type = ANY (ARRAY['global'::text, 'blueprint'::text, 'fleet'::text, 'device'::text]))),
    CONSTRAINT rules_trigger_type_check CHECK ((trigger_type = ANY (ARRAY['telemetry'::text, 'device_status'::text])))
);

CREATE TABLE public.server_config (
    key text NOT NULL,
    value text NOT NULL
);

CREATE TABLE public.server_metrics (
    id bigint NOT NULL,
    cpu_usage_percent real NOT NULL,
    memory_used_bytes bigint NOT NULL,
    memory_total_bytes bigint NOT NULL,
    disk_used_bytes bigint NOT NULL,
    disk_total_bytes bigint NOT NULL,
    network_rx_bytes_delta bigint NOT NULL,
    network_tx_bytes_delta bigint NOT NULL,
    load_avg_1m real NOT NULL,
    load_avg_5m real NOT NULL,
    load_avg_15m real NOT NULL,
    recorded_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE SEQUENCE public.server_metrics_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.server_metrics_id_seq OWNED BY public.server_metrics.id;

CREATE TABLE public.user_roles (
    user_id integer NOT NULL,
    role_id integer NOT NULL,
    tenant_id text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE public.users (
    id integer NOT NULL,
    username text NOT NULL,
    password_hash text NOT NULL,
    role text DEFAULT 'viewer'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    permission_version integer DEFAULT 1 NOT NULL,
    last_login_at timestamp with time zone,
    auth_epoch text DEFAULT (gen_random_uuid())::text NOT NULL,
    CONSTRAINT users_auth_epoch_nonempty CHECK ((auth_epoch <> ''::text))
);

CREATE SEQUENCE public.users_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE public.users_id_seq OWNED BY public.users.id;

CREATE TABLE public.zones (
    id text NOT NULL,
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    geometry_type text NOT NULL,
    geometry_json jsonb NOT NULL,
    color text DEFAULT '#4A90D9'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id text DEFAULT 'default'::text NOT NULL
);

ALTER TABLE ONLY public.api_keys ALTER COLUMN id SET DEFAULT nextval('public.api_keys_id_seq'::regclass);

ALTER TABLE ONLY public.app_metrics ALTER COLUMN id SET DEFAULT nextval('public.app_metrics_id_seq'::regclass);

ALTER TABLE ONLY public.ca_certificates ALTER COLUMN id SET DEFAULT nextval('public.ca_certificates_id_seq'::regclass);

ALTER TABLE ONLY public.device_certificates ALTER COLUMN id SET DEFAULT nextval('public.device_certificates_id_seq'::regclass);

ALTER TABLE ONLY public.device_logs ALTER COLUMN id SET DEFAULT nextval('public.device_logs_id_seq'::regclass);

ALTER TABLE ONLY public.firmware_updates ALTER COLUMN id SET DEFAULT nextval('public.firmware_updates_id_seq'::regclass);

ALTER TABLE ONLY public.fleets ALTER COLUMN id SET DEFAULT nextval('public.fleets_id_seq'::regclass);

ALTER TABLE ONLY public.ota_deployments ALTER COLUMN id SET DEFAULT nextval('public.ota_deployments_id_seq'::regclass);

ALTER TABLE ONLY public.roles ALTER COLUMN id SET DEFAULT nextval('public.roles_id_seq'::regclass);

ALTER TABLE ONLY public.server_metrics ALTER COLUMN id SET DEFAULT nextval('public.server_metrics_id_seq'::regclass);

ALTER TABLE ONLY public.users ALTER COLUMN id SET DEFAULT nextval('public.users_id_seq'::regclass);

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_key_hash_key UNIQUE (key_hash);

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.app_metrics
    ADD CONSTRAINT app_metrics_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.audit_events
    ADD CONSTRAINT audit_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.ca_certificates
    ADD CONSTRAINT ca_certificates_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.command_history
    ADD CONSTRAINT command_history_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_blueprint_drafts
    ADD CONSTRAINT device_blueprint_drafts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_blueprint_drafts
    ADD CONSTRAINT device_blueprint_drafts_tenant_id_blueprint_id_key UNIQUE (tenant_id, blueprint_id);

ALTER TABLE ONLY public.device_blueprint_revisions
    ADD CONSTRAINT device_blueprint_revisions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_blueprint_revisions
    ADD CONSTRAINT device_blueprint_revisions_tenant_id_blueprint_id_revision_key UNIQUE (tenant_id, blueprint_id, revision);

ALTER TABLE ONLY public.device_blueprint_revisions
    ADD CONSTRAINT device_blueprint_revisions_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.device_blueprints
    ADD CONSTRAINT device_blueprints_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_blueprints
    ADD CONSTRAINT device_blueprints_tenant_id_blueprint_key_key UNIQUE (tenant_id, blueprint_key);

ALTER TABLE ONLY public.device_blueprints
    ADD CONSTRAINT device_blueprints_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.device_certificates
    ADD CONSTRAINT device_certificates_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_configs
    ADD CONSTRAINT device_configs_pkey PRIMARY KEY (device_id);

ALTER TABLE ONLY public.device_contract_assignments
    ADD CONSTRAINT device_contract_assignments_pkey PRIMARY KEY (tenant_id, device_id);

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_device_id_key UNIQUE (tenant_id, device_id, id);

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_id_device_id_contract_hash_key UNIQUE (tenant_id, device_id, contract_hash);

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.device_events
    ADD CONSTRAINT device_events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_events
    ADD CONSTRAINT device_events_tenant_id_device_id_id_key UNIQUE (tenant_id, device_id, id);

ALTER TABLE ONLY public.device_logs
    ADD CONSTRAINT device_logs_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.device_metric_samples
    ADD CONSTRAINT device_metric_samples_pkey PRIMARY KEY (event_id, stream_key, field_path);

ALTER TABLE ONLY public.device_shadows
    ADD CONSTRAINT device_shadows_pkey PRIMARY KEY (device_id);

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_tenant_name_key UNIQUE (tenant_id, name);

ALTER TABLE ONLY public.firmware_blobs
    ADD CONSTRAINT firmware_blobs_pkey PRIMARY KEY (firmware_update_id);

ALTER TABLE ONLY public.firmware_updates
    ADD CONSTRAINT firmware_updates_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.firmware_updates
    ADD CONSTRAINT firmware_updates_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.firmware_updates
    ADD CONSTRAINT firmware_updates_tenant_revision_version_key UNIQUE (tenant_id, blueprint_revision_id, version);

ALTER TABLE ONLY public.fleets
    ADD CONSTRAINT fleets_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.fleets
    ADD CONSTRAINT fleets_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.fleets
    ADD CONSTRAINT fleets_tenant_id_name_key UNIQUE (tenant_id, name);

ALTER TABLE ONLY public.organizations
    ADD CONSTRAINT organizations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.role_permissions
    ADD CONSTRAINT role_permissions_pkey PRIMARY KEY (role_id, permission);

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_tenant_id_name_key UNIQUE (tenant_id, name);

ALTER TABLE ONLY public.rule_action_outbox
    ADD CONSTRAINT rule_action_outbox_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rule_actions
    ADD CONSTRAINT rule_actions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rule_alert_deliveries
    ADD CONSTRAINT rule_alert_deliveries_pkey PRIMARY KEY (delivery_id);

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_pkey PRIMARY KEY (rule_id, device_id);

ALTER TABLE ONLY public.rule_zone_entries
    ADD CONSTRAINT rule_zone_entries_pkey PRIMARY KEY (tenant_id, rule_id, device_id);

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.server_config
    ADD CONSTRAINT server_config_pkey PRIMARY KEY (key);

ALTER TABLE ONLY public.server_metrics
    ADD CONSTRAINT server_metrics_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_pkey PRIMARY KEY (user_id, role_id);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_auth_epoch_key UNIQUE (auth_epoch);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_tenant_id_username_key UNIQUE (tenant_id, username);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_tenant_id_id_key UNIQUE (tenant_id, id);

CREATE UNIQUE INDEX firmware_blobs_storage_key_unique ON public.firmware_blobs USING btree (storage_backend, storage_key);

CREATE INDEX idx_alerts_active ON public.alerts USING btree (device_id, created_at) WHERE (status <> 'resolved'::text);

CREATE INDEX idx_alerts_device_id ON public.alerts USING btree (device_id);

CREATE INDEX idx_alerts_rule_id ON public.alerts USING btree (rule_id);

CREATE INDEX idx_alerts_status_created ON public.alerts USING btree (status, created_at);

CREATE INDEX idx_alerts_tenant_device_status ON public.alerts USING btree (tenant_id, device_id, status);

CREATE INDEX idx_api_keys_tenant_blueprint ON public.api_keys USING btree (tenant_id, blueprint_id);

CREATE INDEX idx_api_keys_tenant_id ON public.api_keys USING btree (tenant_id);

CREATE INDEX idx_app_metrics_recorded_at ON public.app_metrics USING btree (recorded_at);

CREATE INDEX idx_app_metrics_recorded_brin ON public.app_metrics USING brin (recorded_at) WITH (pages_per_range='32');

CREATE INDEX idx_audit_events_request_id ON public.audit_events USING btree (request_id);

CREATE INDEX idx_audit_events_tenant_occurred ON public.audit_events USING btree (tenant_id, occurred_at DESC);

CREATE INDEX idx_audit_events_tenant_resource ON public.audit_events USING btree (tenant_id, resource_type, resource_id, occurred_at DESC);

CREATE INDEX idx_command_history_device_created ON public.command_history USING btree (device_id, created_at);

CREATE INDEX idx_command_history_device_id ON public.command_history USING btree (device_id);

CREATE INDEX idx_command_history_status ON public.command_history USING btree (status);

CREATE INDEX idx_command_history_tenant_device_created ON public.command_history USING btree (tenant_id, device_id, created_at);

CREATE INDEX idx_commands_pending ON public.command_history USING btree (created_at) WHERE (status = ANY (ARRAY['sent'::text, 'delivered'::text]));

CREATE INDEX idx_device_blueprint_revisions_latest ON public.device_blueprint_revisions USING btree (tenant_id, blueprint_id, revision DESC);

CREATE INDEX idx_device_blueprints_tenant_name ON public.device_blueprints USING btree (tenant_id, name, id);

CREATE INDEX idx_device_certificates_device_id ON public.device_certificates USING btree (device_id);

CREATE INDEX idx_device_certificates_tenant_device ON public.device_certificates USING btree (tenant_id, device_id);

CREATE INDEX idx_device_configs_tenant_device ON public.device_configs USING btree (tenant_id, device_id);

CREATE INDEX idx_device_contract_assignments_status ON public.device_contract_assignments USING btree (tenant_id, status, updated_at);

CREATE INDEX idx_device_contracts_device_created ON public.device_contracts USING btree (tenant_id, device_id, created_at DESC);

CREATE INDEX idx_device_events_device_time ON public.device_events USING btree (tenant_id, device_id, occurred_at DESC);

CREATE INDEX idx_device_logs_created_brin ON public.device_logs USING brin (created_at) WITH (pages_per_range='32');

CREATE INDEX idx_device_logs_device_created ON public.device_logs USING btree (device_id, created_at);

CREATE INDEX idx_device_logs_device_id ON public.device_logs USING btree (device_id);

CREATE INDEX idx_device_logs_tenant_device_created ON public.device_logs USING btree (tenant_id, device_id, created_at);

CREATE INDEX idx_device_metric_samples_query ON public.device_metric_samples USING btree (tenant_id, device_id, stream_key, field_path, occurred_at DESC);

CREATE INDEX idx_device_shadows_tenant_device ON public.device_shadows USING btree (tenant_id, device_id);

CREATE INDEX idx_devices_declared_connections_gin ON public.devices USING gin (declared_connections);

CREATE INDEX idx_devices_fleet_id ON public.devices USING btree (fleet_id);

CREATE INDEX idx_devices_last_seen ON public.devices USING btree (last_seen);

CREATE INDEX idx_devices_name_trgm ON public.devices USING gin (name public.gin_trgm_ops);

CREATE INDEX idx_devices_online_last_seen ON public.devices USING btree (last_seen) WHERE (status <> 'offline'::text);

CREATE INDEX idx_devices_status ON public.devices USING btree (status);

CREATE INDEX idx_devices_tenant_fleet ON public.devices USING btree (tenant_id, fleet_id);

CREATE INDEX idx_devices_tenant_id ON public.devices USING btree (tenant_id);

CREATE INDEX idx_devices_tenant_status ON public.devices USING btree (tenant_id, status);

CREATE INDEX idx_firmware_blobs_tenant_update ON public.firmware_blobs USING btree (tenant_id, firmware_update_id);

CREATE INDEX idx_firmware_updates_blueprint_revision ON public.firmware_updates USING btree (tenant_id, blueprint_revision_id, created_at DESC);

CREATE INDEX idx_firmware_updates_tenant_id ON public.firmware_updates USING btree (tenant_id);

CREATE INDEX idx_fleets_tenant_id ON public.fleets USING btree (tenant_id);

CREATE INDEX idx_ota_deployments_device_id ON public.ota_deployments USING btree (device_id);

CREATE INDEX idx_ota_deployments_firmware_update_id ON public.ota_deployments USING btree (firmware_update_id);

CREATE INDEX idx_ota_deployments_tenant_device_id ON public.ota_deployments USING btree (tenant_id, device_id);

CREATE INDEX idx_ota_deployments_tenant_update ON public.ota_deployments USING btree (tenant_id, firmware_update_id);

CREATE INDEX idx_role_permissions_permission ON public.role_permissions USING btree (permission);

CREATE INDEX idx_roles_tenant_id ON public.roles USING btree (tenant_id);

CREATE INDEX idx_rule_action_outbox_aggregate ON public.rule_action_outbox USING btree (aggregate_type, aggregate_id, created_at);

CREATE INDEX idx_rule_action_outbox_claim ON public.rule_action_outbox USING btree (status, available_at, created_at) WHERE (status = ANY (ARRAY['pending'::text, 'failed'::text]));

CREATE UNIQUE INDEX idx_rule_action_outbox_idempotency_active ON public.rule_action_outbox USING btree (tenant_id, event_type, idempotency_key) WHERE ((idempotency_key IS NOT NULL) AND (status = ANY (ARRAY['pending'::text, 'processing'::text, 'failed'::text])));

CREATE INDEX idx_rule_action_outbox_processing_lease ON public.rule_action_outbox USING btree (locked_at, created_at) WHERE (status = 'processing'::text);

CREATE INDEX idx_rule_action_outbox_tenant_created ON public.rule_action_outbox USING btree (tenant_id, created_at);

CREATE INDEX idx_rule_actions_rule_id ON public.rule_actions USING btree (rule_id);

CREATE INDEX idx_rule_actions_tenant_rule ON public.rule_actions USING btree (tenant_id, rule_id);

CREATE INDEX idx_rule_conditions_rule_id ON public.rule_conditions USING btree (rule_id);

CREATE INDEX idx_rule_conditions_tenant_rule ON public.rule_conditions USING btree (tenant_id, rule_id);

CREATE INDEX idx_rule_conditions_tenant_zone ON public.rule_conditions USING btree (tenant_id, zone_id);

CREATE INDEX idx_rule_conditions_zone_id ON public.rule_conditions USING btree (zone_id);

CREATE INDEX idx_rule_cooldowns_last_fired_at ON public.rule_cooldowns USING btree (last_fired_at);

CREATE INDEX idx_rule_cooldowns_tenant_device ON public.rule_cooldowns USING btree (tenant_id, device_id);

CREATE INDEX idx_rules_tenant_target ON public.rules USING btree (tenant_id, target_type, target_id);

CREATE INDEX idx_server_metrics_recorded_at ON public.server_metrics USING btree (recorded_at);

CREATE INDEX idx_server_metrics_recorded_brin ON public.server_metrics USING brin (recorded_at) WITH (pages_per_range='32');

CREATE INDEX idx_user_roles_tenant_role ON public.user_roles USING btree (tenant_id, role_id);

CREATE INDEX idx_user_roles_tenant_user ON public.user_roles USING btree (tenant_id, user_id);

CREATE INDEX idx_users_tenant_id ON public.users USING btree (tenant_id);

CREATE INDEX idx_zones_tenant_id ON public.zones USING btree (tenant_id);

CREATE INDEX rule_zone_entries_device ON public.rule_zone_entries USING btree (tenant_id, device_id);

CREATE UNIQUE INDEX rule_zone_entries_devices_tenant_ref ON public.devices USING btree (tenant_id, id);

CREATE UNIQUE INDEX rule_zone_entries_rules_tenant_ref ON public.rules USING btree (tenant_id, id);

CREATE UNIQUE INDEX zones_tenant_name_unique ON public.zones USING btree (tenant_id COLLATE "C", name COLLATE "C");

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.alerts
    ADD CONSTRAINT alerts_tenant_rule_fk FOREIGN KEY (tenant_id, rule_id) REFERENCES public.rules(tenant_id, id) ON DELETE SET NULL (rule_id);

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_tenant_blueprint_fk FOREIGN KEY (tenant_id, blueprint_id) REFERENCES public.device_blueprints(tenant_id, id);

ALTER TABLE ONLY public.api_keys
    ADD CONSTRAINT api_keys_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.device_contract_assignments
    ADD CONSTRAINT assignments_active_contract_fk FOREIGN KEY (tenant_id, device_id, active_contract_id) REFERENCES public.device_contracts(tenant_id, device_id, id);

ALTER TABLE ONLY public.device_contract_assignments
    ADD CONSTRAINT assignments_desired_contract_fk FOREIGN KEY (tenant_id, device_id, desired_contract_id) REFERENCES public.device_contracts(tenant_id, device_id, id);

ALTER TABLE ONLY public.audit_events
    ADD CONSTRAINT audit_events_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.command_history
    ADD CONSTRAINT command_history_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.command_history
    ADD CONSTRAINT command_history_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.command_history
    ADD CONSTRAINT command_history_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.device_blueprint_drafts
    ADD CONSTRAINT device_blueprint_drafts_tenant_id_blueprint_id_fkey FOREIGN KEY (tenant_id, blueprint_id) REFERENCES public.device_blueprints(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_blueprint_revisions
    ADD CONSTRAINT device_blueprint_revisions_tenant_id_blueprint_id_fkey FOREIGN KEY (tenant_id, blueprint_id) REFERENCES public.device_blueprints(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_blueprints
    ADD CONSTRAINT device_blueprints_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_certificates
    ADD CONSTRAINT device_certificates_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_certificates
    ADD CONSTRAINT device_certificates_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_certificates
    ADD CONSTRAINT device_certificates_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.device_configs
    ADD CONSTRAINT device_configs_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_configs
    ADD CONSTRAINT device_configs_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_configs
    ADD CONSTRAINT device_configs_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.device_contract_assignments
    ADD CONSTRAINT device_contract_assignments_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_contract_assignments
    ADD CONSTRAINT device_contract_assignments_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_id_blueprint_revision_id_fkey FOREIGN KEY (tenant_id, blueprint_revision_id) REFERENCES public.device_blueprint_revisions(tenant_id, id);

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_contracts
    ADD CONSTRAINT device_contracts_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_events
    ADD CONSTRAINT device_events_contract_fk FOREIGN KEY (tenant_id, device_id, contract_id) REFERENCES public.device_contracts(tenant_id, device_id, id);

ALTER TABLE ONLY public.device_events
    ADD CONSTRAINT device_events_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_events
    ADD CONSTRAINT device_events_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_logs
    ADD CONSTRAINT device_logs_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_logs
    ADD CONSTRAINT device_logs_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_logs
    ADD CONSTRAINT device_logs_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.device_metric_samples
    ADD CONSTRAINT device_metric_samples_event_fk FOREIGN KEY (tenant_id, device_id, event_id) REFERENCES public.device_events(tenant_id, device_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_metric_samples
    ADD CONSTRAINT device_metric_samples_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_metric_samples
    ADD CONSTRAINT device_metric_samples_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_shadows
    ADD CONSTRAINT device_shadows_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_shadows
    ADD CONSTRAINT device_shadows_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.device_shadows
    ADD CONSTRAINT device_shadows_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_fleet_id_fkey FOREIGN KEY (fleet_id) REFERENCES public.fleets(id) ON DELETE SET NULL;

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_tenant_fleet_fk FOREIGN KEY (tenant_id, fleet_id) REFERENCES public.fleets(tenant_id, id) ON DELETE SET NULL (fleet_id);

ALTER TABLE ONLY public.devices
    ADD CONSTRAINT devices_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.firmware_blobs
    ADD CONSTRAINT firmware_blobs_firmware_update_id_fkey FOREIGN KEY (firmware_update_id) REFERENCES public.firmware_updates(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.firmware_blobs
    ADD CONSTRAINT firmware_blobs_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.firmware_blobs
    ADD CONSTRAINT firmware_blobs_tenant_update_fk FOREIGN KEY (tenant_id, firmware_update_id) REFERENCES public.firmware_updates(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.firmware_updates
    ADD CONSTRAINT firmware_updates_blueprint_revision_fk FOREIGN KEY (tenant_id, blueprint_revision_id) REFERENCES public.device_blueprint_revisions(tenant_id, id);

ALTER TABLE ONLY public.firmware_updates
    ADD CONSTRAINT firmware_updates_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.fleets
    ADD CONSTRAINT fleets_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_firmware_update_id_fkey FOREIGN KEY (firmware_update_id) REFERENCES public.firmware_updates(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.ota_deployments
    ADD CONSTRAINT ota_deployments_tenant_update_fk FOREIGN KEY (tenant_id, firmware_update_id) REFERENCES public.firmware_updates(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.role_permissions
    ADD CONSTRAINT role_permissions_role_id_fkey FOREIGN KEY (role_id) REFERENCES public.roles(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.roles
    ADD CONSTRAINT roles_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_action_outbox
    ADD CONSTRAINT rule_action_outbox_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.rule_actions
    ADD CONSTRAINT rule_actions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_actions
    ADD CONSTRAINT rule_actions_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.rule_actions
    ADD CONSTRAINT rule_actions_tenant_rule_fk FOREIGN KEY (tenant_id, rule_id) REFERENCES public.rules(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_alert_deliveries
    ADD CONSTRAINT rule_alert_deliveries_delivery_id_fkey FOREIGN KEY (delivery_id) REFERENCES public.rule_action_outbox(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_tenant_rule_fk FOREIGN KEY (tenant_id, rule_id) REFERENCES public.rules(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_tenant_zone_fk FOREIGN KEY (tenant_id, zone_id) REFERENCES public.zones(tenant_id, id);

ALTER TABLE ONLY public.rule_conditions
    ADD CONSTRAINT rule_conditions_zone_id_fkey FOREIGN KEY (zone_id) REFERENCES public.zones(id);

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_device_id_fkey FOREIGN KEY (device_id) REFERENCES public.devices(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_rule_id_fkey FOREIGN KEY (rule_id) REFERENCES public.rules(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_tenant_device_fk FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.rule_cooldowns
    ADD CONSTRAINT rule_cooldowns_tenant_rule_fk FOREIGN KEY (tenant_id, rule_id) REFERENCES public.rules(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_zone_entries
    ADD CONSTRAINT rule_zone_entries_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES public.devices(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_zone_entries
    ADD CONSTRAINT rule_zone_entries_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rule_zone_entries
    ADD CONSTRAINT rule_zone_entries_tenant_id_rule_id_fkey FOREIGN KEY (tenant_id, rule_id) REFERENCES public.rules(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.rules
    ADD CONSTRAINT rules_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_role_id_fkey FOREIGN KEY (role_id) REFERENCES public.roles(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_tenant_role_fk FOREIGN KEY (tenant_id, role_id) REFERENCES public.roles(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_tenant_user_fk FOREIGN KEY (tenant_id, user_id) REFERENCES public.users(tenant_id, id) ON DELETE CASCADE;

ALTER TABLE ONLY public.user_roles
    ADD CONSTRAINT user_roles_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

ALTER TABLE ONLY public.zones
    ADD CONSTRAINT zones_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES public.organizations(id);

-- Initial organization and built-in roles; no device families are seeded.
INSERT INTO public.organizations (id, name, created_at, updated_at) VALUES ('default', 'Default Organization', now(), now());

INSERT INTO public.roles (id, tenant_id, name, description, is_system, created_at, updated_at) VALUES (1, 'default', 'owner', 'Full tenant owner with all permissions and lockout protection.', true, now(), now());
INSERT INTO public.roles (id, tenant_id, name, description, is_system, created_at, updated_at) VALUES (2, 'default', 'admin', 'Administrative access to tenant resources and security settings.', true, now(), now());
INSERT INTO public.roles (id, tenant_id, name, description, is_system, created_at, updated_at) VALUES (3, 'default', 'operator', 'Operational access for device management workflows without security administration.', true, now(), now());
INSERT INTO public.roles (id, tenant_id, name, description, is_system, created_at, updated_at) VALUES (4, 'default', 'viewer', 'Read-only operational visibility.', true, now(), now());

INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'zones.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'zones.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'users.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'users.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'telemetry.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'shadows.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'shadows.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'server_metrics.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'rules.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'rules.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'roles.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'roles.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'logs.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'fleets.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'fleets.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'firmware.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'firmware.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'firmware.deploy');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'devices.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'devices.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'commands.send');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'commands.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'alerts.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'alerts.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'api_keys.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'zones.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'zones.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'users.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'users.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'telemetry.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'shadows.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'shadows.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'server_metrics.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'rules.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'rules.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'roles.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'roles.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'logs.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'fleets.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'fleets.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'firmware.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'firmware.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'firmware.deploy');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'devices.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'devices.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'commands.send');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'commands.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'alerts.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'alerts.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'api_keys.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'zones.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'telemetry.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'shadows.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'shadows.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'rules.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'logs.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'fleets.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'firmware.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'devices.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'commands.send');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'commands.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'alerts.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'alerts.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'zones.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'telemetry.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'shadows.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'rules.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'logs.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'fleets.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'firmware.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'devices.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'commands.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'alerts.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'device_blueprints.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (1, 'device_blueprints.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'device_blueprints.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (2, 'device_blueprints.manage');
INSERT INTO public.role_permissions (role_id, permission) VALUES (3, 'device_blueprints.read');
INSERT INTO public.role_permissions (role_id, permission) VALUES (4, 'device_blueprints.read');

SELECT pg_catalog.setval('public.roles_id_seq', 4, true);
