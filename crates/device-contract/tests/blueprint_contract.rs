use std::collections::BTreeMap;

use extrittio_device_contract::{
    BlueprintCompiler, CompileContext, DeviceBlueprint, ResolvedTransport, TransportProtocol,
    validate_blueprint,
};
use serde_json::json;

fn fixture() -> DeviceBlueprint {
    serde_yaml::from_str(include_str!("fixtures/cold-room.yaml")).unwrap()
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
