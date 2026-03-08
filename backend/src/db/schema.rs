// @generated automatically by Diesel CLI.

diesel::table! {
    devices (id) {
        id -> Text,
        name -> Text,
        device_type -> Text,
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

diesel::joinable!(telemetry -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(devices, telemetry,);
