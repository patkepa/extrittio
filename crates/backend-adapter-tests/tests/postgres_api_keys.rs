use diesel::{
    RunQueryDsl,
    r2d2::{ConnectionManager, Pool},
};
use extrittio_backend_postgres::{PostgresApiKeyRepository, run_pending_migrations};

#[tokio::test]
async fn postgres_satisfies_the_api_key_contract_when_configured() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL API-key contract: DATABASE_URL is not set");
        return;
    };
    let pool = Pool::builder()
        .max_size(2)
        .build(ConnectionManager::<diesel::PgConnection>::new(url))
        .expect("connect to disposable PostgreSQL contract database");
    {
        let mut connection = pool.get().unwrap();
        run_pending_migrations(&mut connection).unwrap();
        diesel::sql_query(
            "DELETE FROM api_keys WHERE tenant_id IN ('api-contract-a', 'api-contract-b')",
        )
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query("INSERT INTO organizations (id, name) VALUES ('api-contract-a', 'A'), ('api-contract-b', 'B') ON CONFLICT (id) DO NOTHING").execute(&mut connection).unwrap();
    }
    let repository = PostgresApiKeyRepository::from_pool(pool);
    extrittio_backend_adapter_tests::api_keys::run(&repository).await;
}
