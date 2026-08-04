use async_trait::async_trait;
use diesel::RunQueryDsl;

use crate::persistence::bootstrap::{BootstrapRepository, DatabaseHealth};
use crate::persistence::error::PersistenceError;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

#[derive(diesel::QueryableByName)]
struct HealthRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    value: i32,
}

#[async_trait]
impl BootstrapRepository for PostgresAdapter {
    async fn health(&self) -> Result<DatabaseHealth, PersistenceError> {
        self.executor
            .run(|connection| {
                let row = diesel::sql_query("SELECT 1 AS value")
                    .get_result::<HealthRow>(connection)
                    .map_err(map_diesel_error)?;
                if row.value != 1 {
                    return Err(PersistenceError::CorruptData(
                        "database health query returned an unexpected value".to_string(),
                    ));
                }
                Ok(DatabaseHealth { reachable: true })
            })
            .await
    }
}
