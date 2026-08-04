use chrono::NaiveDateTime;

use crate::error::AppError;

/// Parse an optional timestamp string, accepting both `NaiveDateTime`
/// (`YYYY-MM-DDTHH:MM:SS`) and RFC 3339 formats.
pub fn parse_timestamp(ts_str: Option<&str>) -> Result<Option<NaiveDateTime>, AppError> {
    let Some(s) = ts_str else {
        return Ok(None);
    };
    let dt = s
        .parse::<NaiveDateTime>()
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s).map(|dt| dt.naive_utc()))
        .map_err(|_| {
            AppError::BadRequest("Invalid date format, expected YYYY-MM-DDTHH:MM:SS".into())
        })?;
    Ok(Some(dt))
}
