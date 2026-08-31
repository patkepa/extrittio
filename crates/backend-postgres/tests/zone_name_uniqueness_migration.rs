use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
};

use diesel::connection::SimpleConnection;
use diesel::result::{DatabaseErrorKind, Error};
use diesel::sql_types::Bool;
use diesel::{Connection, PgConnection, QueryableByName, RunQueryDsl};

const MIGRATION_SQL: &str =
    include_str!("../migrations/20260831000000_zone_name_uniqueness/up.sql");

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(0);

struct IsolatedSchema {
    connection: PgConnection,
    name: String,
}

impl IsolatedSchema {
    fn from_environment() -> Option<Self> {
        let database_url = env::var("DATABASE_URL").ok()?;
        let mut connection = PgConnection::establish(&database_url)
            .expect("connect to PostgreSQL for the zone-name migration contract");
        let name = format!(
            "zone_name_migration_{}_{}",
            std::process::id(),
            NEXT_SCHEMA.fetch_add(1, Ordering::Relaxed)
        );

        connection
            .batch_execute(&format!(
                "CREATE SCHEMA {name}; SET search_path TO {name}, public;"
            ))
            .expect("create an isolated PostgreSQL schema");
        connection
            .batch_execute(
                "CREATE TABLE zones (
                    id TEXT PRIMARY KEY NOT NULL,
                    tenant_id TEXT NOT NULL,
                    name TEXT NOT NULL
                );",
            )
            .expect("create the representative pre-migration zones table");

        Some(Self { connection, name })
    }

    fn index_exists(&mut self) -> bool {
        #[derive(QueryableByName)]
        struct IndexPresence {
            #[diesel(sql_type = Bool)]
            present: bool,
        }

        diesel::sql_query(
            "SELECT EXISTS (
                SELECT 1
                  FROM pg_indexes
                 WHERE schemaname = current_schema()
                   AND indexname = 'zones_tenant_name_unique'
            ) AS present",
        )
        .get_result::<IndexPresence>(&mut self.connection)
        .expect("inspect the isolated schema's indexes")
        .present
    }
}

impl Drop for IsolatedSchema {
    fn drop(&mut self) {
        let _ = self.connection.batch_execute(&format!(
            "SET search_path TO public; DROP SCHEMA IF EXISTS {} CASCADE;",
            self.name
        ));
    }
}

#[test]
fn clean_zone_data_upgrades_and_enforces_tenant_name_uniqueness() {
    let Some(mut schema) = IsolatedSchema::from_environment() else {
        eprintln!("skipping PostgreSQL zone-name migration contract: DATABASE_URL is not set");
        return;
    };

    schema
        .connection
        .batch_execute(
            "INSERT INTO zones (id, tenant_id, name) VALUES
                ('zone-a', 'tenant-a', 'North'),
                ('zone-b', 'tenant-b', 'North'),
                ('zone-c', 'tenant-a', 'north');",
        )
        .expect("seed clean representative zone data");

    schema
        .connection
        .transaction::<_, Error, _>(|connection| connection.batch_execute(MIGRATION_SQL))
        .expect("apply the zone-name uniqueness migration");

    assert!(schema.index_exists(), "migration must create the index");

    let error = schema
        .connection
        .batch_execute(
            "INSERT INTO zones (id, tenant_id, name)
             VALUES ('zone-duplicate', 'tenant-a', 'North');",
        )
        .expect_err("the new index must reject a duplicate tenant/name pair");
    assert!(matches!(
        error,
        Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)
    ));
}

#[test]
fn duplicate_zone_data_fails_with_actionable_preflight_and_no_partial_index() {
    let Some(mut schema) = IsolatedSchema::from_environment() else {
        eprintln!("skipping PostgreSQL zone-name migration contract: DATABASE_URL is not set");
        return;
    };

    schema
        .connection
        .batch_execute(
            "INSERT INTO zones (id, tenant_id, name) VALUES
                ('zone-a', 'tenant-a', 'North'),
                ('zone-b', 'tenant-a', 'North');",
        )
        .expect("seed duplicate representative zone data");

    let error = schema
        .connection
        .transaction::<_, Error, _>(|connection| connection.batch_execute(MIGRATION_SQL))
        .expect_err("duplicate zone names must fail the migration preflight");

    let Error::DatabaseError(_, information) = error else {
        panic!("expected a PostgreSQL database error, got {error}");
    };
    assert_eq!(
        information.message(),
        "cannot enforce zone name uniqueness: duplicate (tenant_id, name) rows exist"
    );
    assert_eq!(
        information.hint(),
        Some("Rename or merge duplicate zones, then rerun migrations.")
    );
    assert!(
        !schema.index_exists(),
        "a failed preflight must not leave the uniqueness index behind"
    );
}
