// @generated automatically by Diesel CLI.

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
        location -> Text,
        last_seen -> Nullable<Timestamp>,
        uptime_seconds -> Integer,
        created_at -> Timestamp,
        updated_at -> Timestamp,
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

diesel::joinable!(device_shadows -> devices (device_id));
diesel::joinable!(devices -> device_types (device_type_id));
diesel::joinable!(devices -> fleets (fleet_id));
diesel::joinable!(firmware_updates -> device_types (device_type_id));
diesel::joinable!(ota_deployments -> devices (device_id));
diesel::joinable!(ota_deployments -> firmware_updates (firmware_update_id));
diesel::joinable!(telemetry -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(
    device_shadows,
    device_types,
    devices,
    firmware_updates,
    fleets,
    ota_deployments,
    telemetry,
);
