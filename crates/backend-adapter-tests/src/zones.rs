use extrittio_backend_core::{
    DeleteZoneOutcome, NewZone, PersistenceError, TenantId, ZonePatch, ZoneRepository,
};
use serde_json::json;

use crate::ContractHarness;

/// Execute the canonical zone semantics against one concrete adapter.
///
/// Harnesses must begin with the `tenant-a` and `tenant-b` owners present and
/// no zone rows after `reset`.
pub async fn assert_contract(harness: &dyn ContractHarness) {
    harness.reset().await.expect("reset zone contract fixture");
    let repository = harness.zones();
    let tenant_a = TenantId::new("tenant-a").unwrap();
    let tenant_b = TenantId::new("tenant-b").unwrap();

    let beta = create(&*repository, &tenant_a, "zone-beta", "Beta").await;
    let alpha = create(&*repository, &tenant_a, "zone-alpha", "alpha").await;
    let accent = create(&*repository, &tenant_a, "zone-accent", "éclair").await;
    let other = create(&*repository, &tenant_b, "zone-other", "omega").await;
    let same_name_other_tenant = create(
        &*repository,
        &tenant_b,
        "zone-same-name-other-tenant",
        "Beta",
    )
    .await;

    let duplicate_name = repository
        .create(
            &tenant_a,
            NewZone {
                id: "zone-duplicate-name".into(),
                name: beta.name.clone(),
                description: String::new(),
                geometry_type: "circle".into(),
                geometry_json: json!({"center": [52.0, 21.0], "radius_meters": 25}),
                color: "#4A90D9".into(),
            },
        )
        .await
        .expect_err("zone names are unique within one tenant");
    assert!(
        matches!(
            &duplicate_name,
            PersistenceError::UniqueViolation { constraint }
                if constraint.as_str() == "zones.tenant_name"
        ),
        "unexpected duplicate-name result: {duplicate_name:?}"
    );

    assert_eq!(beta.tenant_id, tenant_a);
    assert_eq!(other.tenant_id, tenant_b);
    assert_eq!(same_name_other_tenant.name, beta.name);
    assert!(
        repository
            .get(&tenant_b, &alpha.id)
            .await
            .expect("cross-tenant get")
            .is_none(),
        "a tenant must never see another tenant's zone"
    );

    let listed = repository.list(&tenant_a).await.expect("list tenant zones");
    assert_eq!(
        listed
            .iter()
            .map(|zone| zone.id.as_str())
            .collect::<Vec<_>>(),
        vec![beta.id.as_str(), alpha.id.as_str(), accent.id.as_str()],
        "zone order is binary name ASC, id ASC"
    );

    let snapshot = harness
        .rule_zone_snapshots()
        .list_for_rule_snapshot()
        .await
        .expect("list system rule-zone snapshot");
    assert_eq!(
        snapshot
            .iter()
            .map(|zone| (zone.tenant_id.as_str(), zone.id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("tenant-a", beta.id.as_str()),
            ("tenant-a", alpha.id.as_str()),
            ("tenant-a", accent.id.as_str()),
            ("tenant-b", same_name_other_tenant.id.as_str()),
            ("tenant-b", other.id.as_str()),
        ],
        "rule snapshots are ordered by binary tenant, name, and id"
    );

    let duplicate_update = repository
        .update(
            &tenant_a,
            &accent.id,
            ZonePatch {
                name: Some(beta.name.clone()),
                description: Some("must roll back".into()),
                geometry_type: None,
                geometry_json: None,
                color: None,
                updated_at: chrono::DateTime::from_timestamp_micros(1_700_000_000_000_000).unwrap(),
            },
        )
        .await
        .expect_err("updating to a duplicate tenant-local name must fail");
    assert!(
        matches!(
            &duplicate_update,
            PersistenceError::UniqueViolation { constraint }
                if constraint.as_str() == "zones.tenant_name"
        ),
        "unexpected duplicate update result: {duplicate_update:?}"
    );
    let unchanged = repository
        .get(&tenant_a, &accent.id)
        .await
        .expect("read after rejected update")
        .expect("zone survives rejected update");
    assert_eq!(unchanged.name, "éclair");
    assert_eq!(unchanged.description, "");

    let updated_at = chrono::DateTime::from_timestamp_micros(1_700_000_000_123_456).unwrap();
    let updated = repository
        .update(
            &tenant_a,
            &alpha.id,
            ZonePatch {
                name: Some("alpha-updated".into()),
                description: Some("changed".into()),
                geometry_type: None,
                geometry_json: None,
                color: Some("#112233".into()),
                updated_at,
            },
        )
        .await
        .expect("update zone")
        .expect("updated zone exists");
    assert_eq!(updated.description, "changed");
    assert_eq!(updated.color, "#112233");
    assert_eq!(updated.geometry_json, alpha.geometry_json);
    assert_eq!(updated.updated_at, updated_at);
    assert_eq!(updated.created_at.timestamp_subsec_nanos() % 1_000, 0);

    assert!(
        repository
            .update(
                &tenant_b,
                &alpha.id,
                ZonePatch {
                    name: Some("forbidden-cross-tenant-write".into()),
                    description: None,
                    geometry_type: None,
                    geometry_json: None,
                    color: None,
                    updated_at: chrono::Utc::now(),
                },
            )
            .await
            .expect("cross-tenant update")
            .is_none()
    );

    assert_eq!(
        repository
            .delete(&tenant_a, "missing-zone")
            .await
            .expect("delete missing zone"),
        DeleteZoneOutcome::NotFound
    );

    harness
        .reference_zone(tenant_a.as_str(), &beta.id)
        .await
        .expect("reference zone from a rule");
    assert_eq!(
        repository
            .delete(&tenant_a, &beta.id)
            .await
            .expect("delete referenced zone"),
        DeleteZoneOutcome::InUse
    );
    assert_eq!(
        repository
            .delete(&tenant_a, &accent.id)
            .await
            .expect("delete unreferenced zone"),
        DeleteZoneOutcome::Deleted
    );
    assert!(
        repository
            .get(&tenant_a, &accent.id)
            .await
            .expect("get deleted zone")
            .is_none()
    );
}

async fn create(
    repository: &dyn ZoneRepository,
    tenant: &TenantId,
    id: &str,
    name: &str,
) -> extrittio_backend_core::Zone {
    repository
        .create(
            tenant,
            NewZone {
                id: id.into(),
                name: name.into(),
                description: String::new(),
                geometry_type: "circle".into(),
                geometry_json: json!({"center": [52.0, 21.0], "radius_meters": 25}),
                color: "#4A90D9".into(),
            },
        )
        .await
        .expect("create zone fixture")
}
