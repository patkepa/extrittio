use std::{
    collections::HashSet,
    env,
    sync::atomic::{AtomicU64, Ordering},
};

use diesel::connection::SimpleConnection;
use diesel::result::{DatabaseErrorKind, Error};
use diesel::sql_types::Text;
use diesel::{Connection, PgConnection, QueryableByName, RunQueryDsl};

const AUTH_EPOCH_SQL: &str = include_str!("../migrations/20260831010000_user_auth_epoch/up.sql");
const NONEMPTY_SQL: &str =
    include_str!("../migrations/20260831020000_user_auth_epoch_nonempty/up.sql");

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(0);

struct IsolatedSchema {
    connection: PgConnection,
    name: String,
}

impl IsolatedSchema {
    fn from_environment() -> Option<Self> {
        let database_url = env::var("DATABASE_URL").ok()?;
        let mut connection = PgConnection::establish(&database_url)
            .expect("connect to PostgreSQL for the user-auth-epoch migration contract");
        let name = format!(
            "user_auth_epoch_migration_{}_{}",
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
                "CREATE TABLE users (
                    id SERIAL PRIMARY KEY,
                    username TEXT NOT NULL UNIQUE
                );",
            )
            .expect("create the representative pre-migration users table");
        Some(Self { connection, name })
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

#[derive(QueryableByName)]
struct EpochRow {
    #[diesel(sql_type = Text)]
    auth_epoch: String,
}

#[test]
fn migration_backfills_unique_epochs_and_enforces_future_values() {
    let Some(mut schema) = IsolatedSchema::from_environment() else {
        eprintln!(
            "skipping PostgreSQL user-auth-epoch migration contract: DATABASE_URL is not set"
        );
        return;
    };
    schema
        .connection
        .batch_execute("INSERT INTO users (username) VALUES ('legacy-a'), ('legacy-b');")
        .expect("seed representative pre-migration users");

    schema
        .connection
        .transaction::<_, Error, _>(|connection| {
            connection.batch_execute(AUTH_EPOCH_SQL)?;
            connection.batch_execute(NONEMPTY_SQL)
        })
        .expect("apply the append-only user authentication epoch migrations");

    let epochs = diesel::sql_query(
        "SELECT auth_epoch FROM users WHERE username IN ('legacy-a', 'legacy-b')",
    )
    .load::<EpochRow>(&mut schema.connection)
    .expect("load backfilled authentication epochs");
    assert_eq!(epochs.len(), 2);
    assert!(epochs.iter().all(|row| !row.auth_epoch.is_empty()));
    assert_eq!(
        epochs
            .iter()
            .map(|row| row.auth_epoch.as_str())
            .collect::<HashSet<_>>()
            .len(),
        2,
        "every existing principal receives a distinct epoch"
    );

    let generated =
        diesel::sql_query("INSERT INTO users (username) VALUES ('new-user') RETURNING auth_epoch")
            .get_result::<EpochRow>(&mut schema.connection)
            .expect("the database default protects non-repository inserts");
    assert!(!generated.auth_epoch.is_empty());

    let duplicate = diesel::sql_query(
        "INSERT INTO users (username, auth_epoch) VALUES ('duplicate-epoch', $1)",
    )
    .bind::<Text, _>(&epochs[0].auth_epoch)
    .execute(&mut schema.connection)
    .expect_err("epochs must be unique across principals");
    assert!(matches!(
        duplicate,
        Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)
    ));

    let empty =
        diesel::sql_query("INSERT INTO users (username, auth_epoch) VALUES ('empty-epoch', '')")
            .execute(&mut schema.connection)
            .expect_err("empty epochs must be rejected");
    let Error::DatabaseError(DatabaseErrorKind::CheckViolation, information) = empty else {
        panic!("expected the auth-epoch check constraint, got {empty}");
    };
    assert_eq!(
        information.constraint_name(),
        Some("users_auth_epoch_nonempty")
    );
}
