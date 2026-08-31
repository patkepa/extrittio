use std::fmt;

/// Validated offset-pagination input used by business read ports.
///
/// HTTP-specific defaults and maximums remain transport policy. This type
/// preserves the existing signed 64-bit values while preventing repositories
/// from receiving a non-positive limit or negative offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    limit: i64,
    offset: i64,
}

impl PageRequest {
    pub fn new(limit: i64, offset: i64) -> Result<Self, PageRequestError> {
        if limit <= 0 {
            return Err(PageRequestError::NonPositiveLimit);
        }
        if offset < 0 {
            return Err(PageRequestError::NegativeOffset);
        }
        Ok(Self { limit, offset })
    }

    #[must_use]
    pub const fn limit(self) -> i64 {
        self.limit
    }

    #[must_use]
    pub const fn offset(self) -> i64 {
        self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageRequestError {
    #[error("page limit must be greater than zero")]
    NonPositiveLimit,
    #[error("page offset must not be negative")]
    NegativeOffset,
}

/// One offset-paginated result and the total matching record count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub records: Vec<T>,
    pub total: i64,
}

impl<T> Page<T> {
    #[must_use]
    pub fn new(records: Vec<T>, total: i64) -> Self {
        Self { records, total }
    }
}

impl fmt::Display for PageRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "limit={}, offset={}", self.limit, self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_request_preserves_valid_i64_values() {
        let request = PageRequest::new(i64::MAX, i64::MAX).unwrap();

        assert_eq!(request.limit(), i64::MAX);
        assert_eq!(request.offset(), i64::MAX);
    }

    #[test]
    fn page_request_rejects_invalid_database_pagination() {
        assert_eq!(
            PageRequest::new(0, 0),
            Err(PageRequestError::NonPositiveLimit)
        );
        assert_eq!(
            PageRequest::new(1, -1),
            Err(PageRequestError::NegativeOffset)
        );
    }
}
