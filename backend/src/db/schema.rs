// @generated automatically by Diesel CLI.

diesel::table! {
    alerts (id) {
        id -> Text,
        rule_id -> Nullable<Text>,
        device_id -> Text,
        severity -> Text,
        status -> Text,
        message -> Text,
        triggered_value -> Nullable<Text>,
        resolved_at -> Nullable<Timestamptz>,
        acknowledged_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_keys (id) {
        id -> Int4,
        name -> Text,
        key_hash -> Text,
        key_prefix -> Text,
        device_type_id -> Nullable<Int4>,
        created_at -> Timestamptz,
        last_used_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    app_metrics (id) {
        id -> Int8,
        request_count -> Int4,
        error_count -> Int4,
        avg_latency_ms -> Float4,
        p95_latency_ms -> Float4,
        db_pool_active -> Int4,
        db_pool_idle -> Int4,
        zenoh_messages_in -> Int4,
        zenoh_messages_out -> Int4,
        recorded_at -> Timestamptz,
    }
}

diesel::table! {
    ca_certificates (id) {
        id -> Int4,
        private_key_pem -> Text,
        certificate_pem -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    command_history (id) {
        id -> Text,
        device_id -> Text,
        command -> Text,
        params -> Text,
        status -> Text,
        response_payload -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    device_certificates (id) {
        id -> Int4,
        device_id -> Text,
        private_key_pem -> Text,
        certificate_pem -> Text,
        fingerprint -> Text,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    device_configs (device_id) {
        device_id -> Text,
        config -> Jsonb,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    device_logs (id) {
        id -> Int8,
        device_id -> Text,
        level -> Text,
        message -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    device_shadows (device_id) {
        device_id -> Text,
        desired -> Jsonb,
        reported -> Jsonb,
        delta -> Jsonb,
        version -> Int4,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    device_types (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    devices (id) {
        id -> Text,
        name -> Text,
        device_type_id -> Int4,
        fleet_id -> Nullable<Int4>,
        status -> Text,
        firmware -> Text,
        last_seen -> Nullable<Timestamptz>,
        uptime_seconds -> Int4,
        latest_latitude -> Nullable<Float8>,
        latest_longitude -> Nullable<Float8>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    firmware_blobs (firmware_update_id) {
        firmware_update_id -> Int4,
        data -> Bytea,
        size -> Int4,
        filename -> Text,
    }
}

diesel::table! {
    firmware_updates (id) {
        id -> Int4,
        device_type_id -> Int4,
        version -> Text,
        url -> Text,
        description -> Nullable<Text>,
        sha256 -> Nullable<Text>,
        commit_sha -> Nullable<Text>,
        branch -> Nullable<Text>,
        ci_run_url -> Nullable<Text>,
        build_timestamp -> Nullable<Timestamptz>,
        changelog -> Nullable<Text>,
        source -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    fleets (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    ota_deployments (id) {
        id -> Int4,
        device_id -> Text,
        firmware_update_id -> Int4,
        status -> Text,
        error_message -> Nullable<Text>,
        initiated_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    rule_actions (id) {
        id -> Text,
        rule_id -> Text,
        action_type -> Text,
        config -> Jsonb,
    }
}

diesel::table! {
    rule_conditions (id) {
        id -> Text,
        rule_id -> Text,
        field -> Text,
        operator -> Text,
        value -> Text,
        condition_group -> Int4,
        zone_id -> Nullable<Text>,
    }
}

diesel::table! {
    rule_cooldowns (rule_id, device_id) {
        rule_id -> Text,
        device_id -> Text,
        last_fired_at -> Timestamptz,
    }
}

diesel::table! {
    rules (id) {
        id -> Text,
        name -> Text,
        description -> Nullable<Text>,
        enabled -> Bool,
        trigger_type -> Text,
        target_type -> Text,
        target_id -> Nullable<Text>,
        cooldown_seconds -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    server_config (key) {
        key -> Text,
        value -> Text,
    }
}

diesel::table! {
    server_metrics (id) {
        id -> Int8,
        cpu_usage_percent -> Float4,
        memory_used_bytes -> Int8,
        memory_total_bytes -> Int8,
        disk_used_bytes -> Int8,
        disk_total_bytes -> Int8,
        network_rx_bytes_delta -> Int8,
        network_tx_bytes_delta -> Int8,
        load_avg_1m -> Float4,
        load_avg_5m -> Float4,
        load_avg_15m -> Float4,
        recorded_at -> Timestamptz,
    }
}

diesel::table! {
    telemetry (id) {
        id -> Int8,
        device_id -> Text,
        payload -> Bytea,
        temperature -> Nullable<Float4>,
        humidity -> Nullable<Float4>,
        battery_level -> Nullable<Float4>,
        custom_json -> Nullable<Jsonb>,
        latitude -> Nullable<Float8>,
        longitude -> Nullable<Float8>,
        speed -> Nullable<Float4>,
        altitude -> Nullable<Float4>,
        heading -> Nullable<Float4>,
        received_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Text,
        password_hash -> Text,
        role -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    zones (id) {
        id -> Text,
        name -> Text,
        description -> Text,
        geometry_type -> Text,
        geometry_json -> Jsonb,
        color -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(alerts -> devices (device_id));
diesel::joinable!(alerts -> rules (rule_id));
diesel::joinable!(api_keys -> device_types (device_type_id));
diesel::joinable!(command_history -> devices (device_id));
diesel::joinable!(device_certificates -> devices (device_id));
diesel::joinable!(device_configs -> devices (device_id));
diesel::joinable!(device_logs -> devices (device_id));
diesel::joinable!(device_shadows -> devices (device_id));
diesel::joinable!(devices -> device_types (device_type_id));
diesel::joinable!(devices -> fleets (fleet_id));
diesel::joinable!(firmware_blobs -> firmware_updates (firmware_update_id));
diesel::joinable!(firmware_updates -> device_types (device_type_id));
diesel::joinable!(ota_deployments -> devices (device_id));
diesel::joinable!(ota_deployments -> firmware_updates (firmware_update_id));
diesel::joinable!(rule_actions -> rules (rule_id));
diesel::joinable!(rule_conditions -> rules (rule_id));
diesel::joinable!(rule_conditions -> zones (zone_id));
diesel::joinable!(rule_cooldowns -> devices (device_id));
diesel::joinable!(rule_cooldowns -> rules (rule_id));
diesel::joinable!(telemetry -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(
    alerts,
    api_keys,
    app_metrics,
    ca_certificates,
    command_history,
    device_certificates,
    device_configs,
    device_logs,
    device_shadows,
    device_types,
    devices,
    firmware_blobs,
    firmware_updates,
    fleets,
    ota_deployments,
    rule_actions,
    rule_conditions,
    rule_cooldowns,
    rules,
    server_config,
    server_metrics,
    telemetry,
    users,
    zones,
);
