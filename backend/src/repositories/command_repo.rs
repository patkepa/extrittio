// Repository functions for commands

use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{CommandRecord, NewCommandRecord};
use crate::db::schema::command_history;

pub fn insert_command(
    conn: &mut SqliteConnection,
    record: &NewCommandRecord,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(command_history::table)
        .values(record)
        .execute(conn)?;
    Ok(())
}

pub fn find_command(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<CommandRecord, diesel::result::Error> {
    command_history::table
        .find(id)
        .select(CommandRecord::as_select())
        .first(conn)
}

pub fn update_command_status(
    conn: &mut SqliteConnection,
    id: &str,
    status: &str,
    response_payload: Option<&str>,
    updated_at: chrono::NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(command_history::table.find(id))
        .set((
            command_history::status.eq(status),
            command_history::response_payload.eq(response_payload),
            command_history::updated_at.eq(updated_at),
        ))
        .execute(conn)
}

/// Bulk-update stale sent/delivered commands to "timed_out".
pub fn timeout_stale_commands(
    conn: &mut SqliteConnection,
    cutoff: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::update(
        command_history::table
            .filter(command_history::status.eq_any(&["sent", "delivered"]))
            .filter(command_history::created_at.lt(cutoff)),
    )
    .set((
        command_history::status.eq("timed_out"),
        command_history::updated_at.eq(now),
    ))
    .execute(conn)
}

pub fn list_commands(
    conn: &mut SqliteConnection,
    device_id: &str,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<CommandRecord>, diesel::result::Error> {
    let mut query = command_history::table
        .filter(command_history::device_id.eq(device_id))
        .into_boxed();

    if let Some(status) = status {
        query = query.filter(command_history::status.eq(status));
    }

    query
        .order(command_history::created_at.desc())
        .limit(limit)
        .select(CommandRecord::as_select())
        .load(conn)
}
