// @generated automatically by Diesel CLI.

diesel::table! {
    api_keys (id) {
        id -> Integer,
        name -> Text,
        key_hash -> Text,
        key_prefix -> Text,
        device_type_id -> Nullable<Integer>,
        created_at -> Timestamp,
        last_used_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    app_metrics (id) {
        id -> Integer,
        request_count -> Integer,
        error_count -> Integer,
        avg_latency_ms -> Float,
        p95_latency_ms -> Float,
        db_pool_active -> Integer,
        db_pool_idle -> Integer,
        zenoh_messages_in -> Integer,
        zenoh_messages_out -> Integer,
        recorded_at -> Timestamp,
    }
}

diesel::table! {
    ca_certificates (id) {
        id -> Integer,
        private_key_pem -> Text,
        certificate_pem -> Text,
        created_at -> Timestamp,
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
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    device_certificates (id) {
        id -> Integer,
        device_id -> Text,
        private_key_pem -> Text,
        certificate_pem -> Text,
        fingerprint -> Text,
        expires_at -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::table! {
    device_configs (device_id) {
        device_id -> Text,
        config -> Text,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    device_logs (id) {
        id -> Integer,
        device_id -> Text,
        level -> Text,
        message -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    device_shadows (device_id) {
        device_id -> Text,
        desired -> Text,
        reported -> Text,
        delta -> Text,
        version -> Integer,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    device_types (id) {
        id -> Integer,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    devices (id) {
        id -> Text,
        name -> Text,
        device_type_id -> Integer,
        fleet_id -> Nullable<Integer>,
        status -> Text,
        firmware -> Text,
        last_seen -> Nullable<Timestamp>,
        uptime_seconds -> Integer,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    firmware_blobs (firmware_update_id) {
        firmware_update_id -> Integer,
        data -> Binary,
        size -> Integer,
        filename -> Text,
    }
}

diesel::table! {
    firmware_updates (id) {
        id -> Integer,
        device_type_id -> Integer,
        version -> Text,
        url -> Text,
        description -> Nullable<Text>,
        created_at -> Timestamp,
        sha256 -> Nullable<Text>,
        commit_sha -> Nullable<Text>,
        branch -> Nullable<Text>,
        ci_run_url -> Nullable<Text>,
        build_timestamp -> Nullable<Timestamp>,
        changelog -> Nullable<Text>,
        source -> Text,
    }
}

diesel::table! {
    fleets (id) {
        id -> Integer,
        name -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    ota_deployments (id) {
        id -> Integer,
        device_id -> Text,
        firmware_update_id -> Integer,
        status -> Text,
        error_message -> Nullable<Text>,
        initiated_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
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
        id -> Integer,
        cpu_usage_percent -> Float,
        memory_used_bytes -> BigInt,
        memory_total_bytes -> BigInt,
        disk_used_bytes -> BigInt,
        disk_total_bytes -> BigInt,
        network_rx_bytes_delta -> BigInt,
        network_tx_bytes_delta -> BigInt,
        load_avg_1m -> Float,
        load_avg_5m -> Float,
        load_avg_15m -> Float,
        recorded_at -> Timestamp,
    }
}

diesel::table! {
    telemetry (id) {
        id -> Integer,
        device_id -> Text,
        payload -> Binary,
        temperature -> Nullable<Float>,
        humidity -> Nullable<Float>,
        battery_level -> Nullable<Float>,
        custom_json -> Nullable<Text>,
        received_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password_hash -> Text,
        role -> Text,
        created_at -> Timestamp,
    }
}

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
diesel::joinable!(telemetry -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(
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
    server_config,
    server_metrics,
    telemetry,
    users,
);
