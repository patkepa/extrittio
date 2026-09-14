use crate::error::AppError;
use extrittio_backend_core::rule_engine::types::PendingAction;

pub(super) fn enqueue_pending_actions(
    connection: &mut diesel::PgConnection,
    actions: &[PendingAction],
) -> Result<usize, AppError> {
    Ok(crate::database::postgres_enqueue_actions(
        connection, actions,
    )?)
}
