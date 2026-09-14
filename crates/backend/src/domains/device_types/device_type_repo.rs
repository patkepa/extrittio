//! Temporary R09 firmware lookup; removed with the remaining firmware adapter.
use crate::db::{models::DeviceType, schema::device_types};
use diesel::prelude::*;

pub fn find_device_type_by_id(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: i32,
) -> Result<DeviceType, diesel::result::Error> {
    device_types::table
        .filter(device_types::tenant_id.eq(tenant_id))
        .filter(device_types::id.eq(id))
        .select(DeviceType::as_select())
        .first(conn)
}
