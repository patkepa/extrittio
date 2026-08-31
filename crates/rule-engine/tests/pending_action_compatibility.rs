use std::collections::BTreeSet;

use extrittio_rule_engine::types::PendingAction;

const LEGACY_V0: &str = include_str!("fixtures/pending-actions-v0.json");

#[test]
fn pending_action_v0_fixture_round_trips_without_shape_changes() {
    let expected: serde_json::Value = serde_json::from_str(LEGACY_V0).unwrap();
    let actions: Vec<PendingAction> = serde_json::from_value(expected.clone()).unwrap();

    assert_eq!(serde_json::to_value(actions).unwrap(), expected);
}

#[test]
fn pending_action_v0_fixture_covers_every_current_variant() {
    let actions: Vec<serde_json::Value> = serde_json::from_str(LEGACY_V0).unwrap();
    let variants = actions
        .iter()
        .map(|action| {
            action
                .as_object()
                .and_then(|object| object.keys().next())
                .map(String::as_str)
                .unwrap()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        variants,
        [
            "CreateAlert",
            "ResolveAlert",
            "SendCommand",
            "SendWebhook",
            "UpdateAlertValue",
            "UpdateCooldown",
            "UpdateZoneEntry",
        ]
        .into_iter()
        .collect()
    );
}
