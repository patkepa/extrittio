use extrittio_backend_turso::{TursoApiKeyRepository, TursoDatabase};
use std::time::Duration;

#[tokio::test]
async fn turso_satisfies_the_api_key_contract() {
    let directory = tempfile::tempdir().unwrap();
    let database = TursoDatabase::open_and_migrate(
        directory.path(),
        &directory.path().join("api-keys.db"),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    let handles = database.shared_handles();
    {
        let writer = handles.lock_writer().await;
        writer.execute_batch("INSERT INTO organizations (id, name, created_at, updated_at) VALUES ('api-contract-a', 'A', 1, 1), ('api-contract-b', 'B', 1, 1);").await.unwrap();
    }
    let repository = TursoApiKeyRepository::from_handles(handles);
    extrittio_backend_adapter_tests::api_keys::run(&repository).await;
}
