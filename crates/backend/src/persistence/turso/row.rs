//! Compatibility names for adapter-owned Turso row decoding.

pub(crate) use crate::database::{
    turso_datetime as datetime, turso_error as error, turso_i32 as i32,
};
