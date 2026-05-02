use std::fmt::Write as _;

use crate::args::{ListDevicesArgs, PageArgs};

pub(super) fn page_query(args: &PageArgs) -> String {
    format!("?limit={}&offset={}", args.limit, args.offset)
}

pub(super) fn device_list_query(args: &ListDevicesArgs) -> String {
    let mut params = vec![
        format!("limit={}", args.limit),
        format!("offset={}", args.offset),
    ];
    if let Some(status) = &args.status {
        params.push(format!("status={}", percent_encode(status)));
    }
    if let Some(search) = &args.search {
        params.push(format!("search={}", percent_encode(search)));
    }
    if let Some(fleet_id) = args.fleet_id {
        params.push(format!("fleet_id={fleet_id}"));
    }
    format!("?{}", params.join("&"))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use crate::args::{ListDevicesArgs, PageArgs};

    use super::{device_list_query, page_query};

    #[test]
    fn page_query_includes_limit_and_offset() {
        assert_eq!(
            page_query(&PageArgs {
                limit: 25,
                offset: 50,
            }),
            "?limit=25&offset=50"
        );
    }

    #[test]
    fn device_list_query_encodes_optional_filters() {
        let query = device_list_query(&ListDevicesArgs {
            status: Some("online".to_string()),
            search: Some("floor 1".to_string()),
            fleet_id: Some(7),
            limit: 10,
            offset: 20,
        });

        assert_eq!(
            query,
            "?limit=10&offset=20&status=online&search=floor%201&fleet_id=7"
        );
    }
}
