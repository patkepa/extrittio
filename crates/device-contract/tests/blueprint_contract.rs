use std::collections::BTreeMap;

use extrittio_device_contract::{
    BlueprintCompiler, CompileContext, DeviceBlueprint, ResolvedTransport, TransportProtocol,
    validate_blueprint,
};
use serde_json::json;

fn fixture() -> DeviceBlueprint {
    serde_yaml::from_str(include_str!("fixtures/cold-room.yaml")).unwrap()
}

fn starter_fixture() -> DeviceBlueprint {
    serde_json::from_str(include_str!(
        "../../../apps/frontend/src/pages/settings/starter-device-blueprint.json"
    ))
    .unwrap()
}

fn located_fixture() -> DeviceBlueprint {
    let mut blueprint = fixture();
    let stream = &mut blueprint.spec.streams[0];
    for path in ["/position/north", "/position/east"] {
        stream.fields.push(
            serde_json::from_value(json!({
                "path": path, "type": "float64", "label": path, "unit": "deg"
            }))
            .unwrap(),
        );
    }
    blueprint.spec.location = Some(
        serde_json::from_value(json!({
            "stream": stream.key,
            "latitudePath": "/position/north",
            "longitudePath": "/position/east",
            "coordinateSystem": "wgs84",
            "unit": "degrees",
            "maxAge": "30s"
        }))
        .unwrap(),
    );
    blueprint
}

#[test]
fn validates_location_references_and_freshness() {
    validate_blueprint(located_fixture()).unwrap();
    for (property, value) in [
        ("stream", "unknown"),
        ("latitudePath", "/missing"),
        ("longitudePath", "/position/north"),
        ("maxAge", "0s"),
    ] {
        let mut document = serde_json::to_value(located_fixture()).unwrap();
        document["spec"]["location"][property] = json!(value);
        assert!(
            validate_blueprint(serde_json::from_value(document).unwrap()).is_err(),
            "{property}"
        );
    }
    let mut blueprint = located_fixture();
    blueprint.spec.streams[0]
        .fields
        .last_mut()
        .unwrap()
        .value_type = extrittio_device_contract::FieldValueType::String;
    assert!(validate_blueprint(blueprint).is_err());
}

#[test]
fn changing_location_binding_is_a_breaking_blueprint_revision() {
    let original = validate_blueprint(located_fixture()).unwrap();
    let mut changed = located_fixture();
    changed.spec.location.as_mut().unwrap().max_age = "10s".into();
    let comparison = extrittio_device_contract::compare_blueprints(
        &original,
        &validate_blueprint(changed).unwrap(),
    );
    assert!(comparison.breaking);
    assert!(
        comparison
            .changes
            .iter()
            .any(|change| change.path == "/spec/location")
    );
}

#[test]
fn location_uses_one_fresh_event_and_preserves_origin() {
    let blueprint = located_fixture();
    let route = blueprint.spec.streams[0].route.clone();
    let context = CompileContext {
        contract_id: "location-contract".into(),
        tenant_id: "tenant-a".into(),
        device_id: "position-1".into(),
        blueprint_revision_id: "revision-1".into(),
        blueprint_revision: 1,
        transport_bindings: BTreeMap::from([(
            "primary_zenoh".into(),
            ResolvedTransport {
                protocol: TransportProtocol::Zenoh,
                endpoint: "tcp/hub.internal:7447".into(),
                server_ca_pem: None,
                credential_ref: None,
            },
        )]),
        configuration_layers: vec![],
    };
    let compiled =
        BlueprintCompiler::compile(&validate_blueprint(blueprint).unwrap(), &context).unwrap();
    assert!(compiled.verify_hash().unwrap());
    let contract = compiled.document;
    let origin = json!({"position": {"north": 0, "east": 0}});
    assert_eq!(
        contract.event_location(&route, &origin, 30_000),
        Some((0.0, 0.0))
    );
    assert_eq!(contract.event_location(&route, &origin, 30_001), None);
    assert_eq!(contract.event_location(&route, &origin, -1), None);
    assert_eq!(contract.event_location("unrelated-route", &origin, 0), None);
    for payload in [
        json!({"position": {"north": 1}}),
        json!({"position": {"east": 2}}),
        json!({"position": {"north": null, "east": 2}}),
        json!({"position": {"north": 91, "east": 2}}),
        json!({"position": {"north": 1, "east": 181}}),
        json!({"position": {"north": "1", "east": 2}}),
    ] {
        assert_eq!(contract.event_location(&route, &payload, 0), None);
    }
}

#[test]
fn blueprint_fixture_validates_and_compiles() {
    let blueprint = validate_blueprint(fixture()).unwrap();
    let context = CompileContext {
        contract_id: "dc_01".to_string(),
        tenant_id: "tenant-a".to_string(),
        device_id: "sensor-17".to_string(),
        blueprint_revision_id: "dbr_01".to_string(),
        blueprint_revision: 1,
        transport_bindings: BTreeMap::from([(
            "primary_zenoh".to_string(),
            ResolvedTransport {
                protocol: TransportProtocol::Zenoh,
                endpoint: "tcp/hub.internal:7447".to_string(),
                server_ca_pem: Some("secret-ref:hub-ca".to_string()),
                credential_ref: Some("device-certificate".to_string()),
            },
        )]),
        configuration_layers: vec![
            json!({"sample_interval_seconds": 45}),
            json!({"temperature_offset": -0.25}),
        ],
    };

    let contract = BlueprintCompiler::compile(&blueprint, &context).unwrap();
    assert!(contract.verify_hash().unwrap());
    assert_eq!(contract.document.device_id, "sensor-17");
    assert_eq!(
        contract.document.configuration.unwrap().desired,
        json!({"sample_interval_seconds": 45, "temperature_offset": -0.25})
    );
}

#[test]
fn reports_all_structural_validation_errors() {
    let mut blueprint = fixture();
    blueprint.api_version = "unknown/v9".to_string();
    blueprint.spec.routes[0].transport = "missing".to_string();
    blueprint.spec.streams[0].fields[0].path = "not-a-pointer".to_string();

    let error = validate_blueprint(blueprint).unwrap_err();
    let issues = error.validation_issues().unwrap();
    assert!(issues.len() >= 3);
    assert!(issues.iter().any(|issue| issue.path == "/apiVersion"));
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == "/spec/routes/0/transport")
    );
}

#[test]
fn frontend_starter_blueprint_validates_and_compiles() {
    let blueprint = validate_blueprint(starter_fixture()).unwrap();
    let context = CompileContext {
        contract_id: "dc_starter".to_string(),
        tenant_id: "tenant-a".to_string(),
        device_id: "starter-device".to_string(),
        blueprint_revision_id: "dbr_starter".to_string(),
        blueprint_revision: 1,
        transport_bindings: BTreeMap::from([(
            "site_bus".to_string(),
            ResolvedTransport {
                protocol: TransportProtocol::Zenoh,
                endpoint: "tcp/hub.internal:7447".to_string(),
                server_ca_pem: None,
                credential_ref: Some("device-certificate".to_string()),
            },
        )]),
        configuration_layers: Vec::new(),
    };

    let contract = BlueprintCompiler::compile(&blueprint, &context).unwrap();

    assert_eq!(
        contract.document.presentation.unwrap().summary[0].metric,
        "environment./temperature"
    );
}
