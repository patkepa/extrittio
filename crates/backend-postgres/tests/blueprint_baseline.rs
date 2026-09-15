use diesel::connection::SimpleConnection;
use diesel::{Connection, PgConnection};

const BASELINE: &str = include_str!("../migrations/00000000000001_blueprint_baseline/up.sql");
const ASSERTIONS: &str = include_str!("blueprint_baseline_assertions.sql");

#[test]
fn baseline_excludes_retired_device_schema() {
    for retired in [
        "CREATE TABLE public.device_types",
        "CREATE TABLE public.telemetry",
        "CREATE TABLE public.device_latest_state",
        "CREATE TABLE public.rule_zone_handoffs",
        "CREATE TABLE public.rule_cooldown_resets",
        "CREATE TABLE public.network_observed_hosts",
        "device_type_id",
        "latest_latitude",
        "latest_longitude",
        "'device_type'",
        "'device_types.read'",
        "'device_types.manage'",
    ] {
        assert!(!BASELINE.contains(retired), "retired schema: {retired}");
    }
    assert_eq!(
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
            .unwrap()
            .count(),
        1,
        "only the fresh baseline belongs in the migration directory"
    );
}

// Explicitly ignored rather than silently passing when PostgreSQL is unavailable.
// Use a dedicated empty database; all DDL and fixture writes are rolled back.
#[test]
#[ignore = "requires EXTRITTIO_TEST_EMPTY_POSTGRES_URL pointing to a disposable empty database"]
fn empty_database_runs_baseline_and_enforces_constraints() {
    let url = std::env::var("EXTRITTIO_TEST_EMPTY_POSTGRES_URL").unwrap();
    let mut connection = PgConnection::establish(&url).unwrap();
    connection.test_transaction::<_, diesel::result::Error, _>(|connection| {
        let versions = extrittio_backend_postgres::run_pending_migrations(connection).unwrap();
        assert_eq!(versions, ["00000000000001"]);
        assert!(
            extrittio_backend_postgres::run_pending_migrations(connection)
                .unwrap()
                .is_empty()
        );
        connection.batch_execute(ASSERTIONS)?;
        Ok(())
    });
}
