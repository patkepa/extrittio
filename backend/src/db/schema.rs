// @generated automatically by Diesel CLI.

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
    fleets (id) {
        id -> Integer,
        name -> Text,
        created_at -> Timestamp,
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

diesel::joinable!(devices -> device_types (device_type_id));
diesel::joinable!(devices -> fleets (fleet_id));
diesel::joinable!(telemetry -> devices (device_id));

diesel::allow_tables_to_appear_in_same_query!(device_types, devices, fleets, telemetry,);
