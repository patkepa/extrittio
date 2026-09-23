use extrittio_backend_core::{ApiKeyRepository, CreateApiKeyRecord, PersistenceError, TenantId};

/// Both adapters must preserve tenant filtering, exact names, ordering and missing-delete behavior.
pub async fn run(repository: &dyn ApiKeyRepository) {
    let tenant_a = TenantId::new("api-contract-a").unwrap();
    let tenant_b = TenantId::new("api-contract-b").unwrap();
    let create = |name: &str, hash: &str| CreateApiKeyRecord {
        name: name.into(),
        key_hash: hash.into(),
        key_prefix: "extr_display".into(),
        blueprint_id: None,
    };
    let a = repository
        .create(&tenant_a, create(" Same name ", "api-contract-a1"))
        .await
        .unwrap();
    let b = repository
        .create(&tenant_b, create(" Same name ", "api-contract-b1"))
        .await
        .unwrap();
    let later = repository
        .create(&tenant_a, create("Later key", "api-contract-a2"))
        .await
        .unwrap();
    assert_eq!(a.name, " Same name ");
    assert_eq!(a.key_prefix, "extr_display");
    assert_eq!(a.last_used_at, None);
    let listed = repository.list(&tenant_a).await.unwrap();
    assert_eq!(
        listed.iter().map(|entry| entry.key.id).collect::<Vec<_>>(),
        [later.id, a.id]
    );
    assert!(listed.iter().all(|entry| entry.blueprint_name.is_none()));
    assert_eq!(repository.list(&tenant_b).await.unwrap()[0].key.id, b.id);
    assert!(!repository.delete(&tenant_b, a.id).await.unwrap());
    assert_eq!(repository.list(&tenant_a).await.unwrap().len(), 2);
    assert!(matches!(
        repository
            .create(&tenant_a, create("Duplicate hash", "api-contract-a1"))
            .await,
        Err(PersistenceError::UniqueViolation { .. })
    ));
    assert!(repository.delete(&tenant_a, a.id).await.unwrap());
    assert!(!repository.delete(&tenant_a, a.id).await.unwrap());
    assert_eq!(repository.list(&tenant_a).await.unwrap().len(), 1);
    assert!(repository.delete(&tenant_a, later.id).await.unwrap());
    assert!(repository.delete(&tenant_b, b.id).await.unwrap());
}
