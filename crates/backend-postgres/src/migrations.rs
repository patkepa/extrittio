//! PostgreSQL migration assets owned by this adapter's lifecycle surface.

use diesel::PgConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

#[derive(Debug, thiserror::Error)]
#[error("PostgreSQL migration failed: {source}")]
pub struct PostgresMigrationError {
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn run_pending_migrations(
    connection: &mut PgConnection,
) -> Result<Vec<String>, PostgresMigrationError> {
    connection
        .run_pending_migrations(MIGRATIONS)
        .map(|versions| {
            versions
                .into_iter()
                .map(|version| version.to_string())
                .collect()
        })
        .map_err(|source| PostgresMigrationError { source })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use diesel::migration::MigrationSource;
    use diesel::pg::Pg;
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn embeds_the_complete_legacy_postgres_migration_chain() {
        let migrations = <EmbeddedMigrations as MigrationSource<Pg>>::migrations(&MIGRATIONS)
            .expect("embedded migrations are valid");
        let versions = migrations
            .iter()
            .map(|migration| migration.name().version().to_string())
            .collect::<Vec<_>>();

        assert_eq!(versions.len(), 26);
        assert_eq!(versions.first().map(String::as_str), Some("00000000000000"));
        assert_eq!(versions.last().map(String::as_str), Some("20260831010000"));
    }

    #[test]
    fn migration_assets_match_the_compatibility_manifest() {
        let expected = include_str!("../tests/fixtures/migration-checksums-v1.txt")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let mut fields = line.split_whitespace();
                let version = fields.next().unwrap();
                let up = fields.next().unwrap();
                let down = fields.next().unwrap();
                assert!(fields.next().is_none(), "invalid fixture line: {line}");
                (version, up, down)
            })
            .collect::<Vec<_>>();

        let migrations = <EmbeddedMigrations as MigrationSource<Pg>>::migrations(&MIGRATIONS)
            .expect("embedded migrations are valid");
        let versions = migrations
            .iter()
            .map(|migration| migration.name().version().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            versions,
            expected
                .iter()
                .map(|(version, _, _)| version.split_once('_').unwrap().0.to_string())
                .collect::<Vec<_>>()
        );

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        for (version, expected_up, expected_down) in expected {
            let migration = root.join(version);
            assert_eq!(
                sha256(&migration.join("up.sql")),
                expected_up,
                "{version}/up.sql"
            );
            assert_eq!(
                sha256(&migration.join("down.sql")),
                expected_down,
                "{version}/down.sql"
            );
        }
    }

    fn sha256(path: &Path) -> String {
        format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
    }
}
