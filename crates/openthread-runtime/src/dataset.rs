use std::collections::BTreeSet;
use zeroize::Zeroizing;

use crate::{
    error::{OpenThreadError, Result},
    types::{ThreadActiveDataset, ValidatedCreateNetwork},
};

const MAX_OPERATIONAL_DATASET_BYTES: usize = 254;
const CHANNEL_TLV: u8 = 0;
const PAN_ID_TLV: u8 = 1;
const EXTENDED_PAN_ID_TLV: u8 = 2;
const NETWORK_NAME_TLV: u8 = 3;
const PSKC_TLV: u8 = 4;
const NETWORK_KEY_TLV: u8 = 5;
const MESH_LOCAL_PREFIX_TLV: u8 = 7;
const SECURITY_POLICY_TLV: u8 = 12;
const CHANNEL_MASK_TLV: u8 = 53;
const REQUIRED_ACTIVE_DATASET_TLVS: [u8; 10] = [0, 1, 2, 3, 4, 5, 7, 12, 14, 53];

pub(crate) fn normalize_create_network(
    network_name: String,
    channel: Option<u16>,
    pan_id: Option<String>,
    extended_pan_id: Option<String>,
    network_key: Option<String>,
) -> Result<ValidatedCreateNetwork> {
    if network_name.trim() != network_name {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread network name must not have leading or trailing whitespace".to_string(),
        ));
    }
    if network_name.is_empty() || network_name.len() > 16 || !network_name.is_ascii() {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread network name must be 1 to 16 ASCII characters".to_string(),
        ));
    }
    if network_name.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread network name must not contain control characters".to_string(),
        ));
    }
    if channel.is_some_and(|channel| !(11..=26).contains(&channel)) {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread channel must be between 11 and 26".to_string(),
        ));
    }
    let pan_id = normalize_optional_hex("Thread PAN ID", pan_id, 4)?;
    if pan_id.as_deref() == Some("ffff") {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread PAN ID must not use the reserved broadcast value ffff".to_string(),
        ));
    }
    let extended_pan_id = normalize_optional_hex("Thread extended PAN ID", extended_pan_id, 16)?;
    if extended_pan_id
        .as_deref()
        .is_some_and(|value| value == "0000000000000000" || value == "ffffffffffffffff")
    {
        return Err(OpenThreadError::InvalidConfiguration(
            "Thread extended PAN ID must not be all-zero or all-ones".to_string(),
        ));
    }
    let network_key = normalize_optional_secret_hex("Thread network key", network_key, 32)?;
    Ok(ValidatedCreateNetwork {
        network_name,
        channel,
        pan_id,
        extended_pan_id,
        network_key,
    })
}

pub(crate) fn parse_pan_id(value: &str) -> Result<u16> {
    u16::from_str_radix(strip_hex_prefix(value), 16).map_err(|_| {
        OpenThreadError::InvalidConfiguration("Thread PAN ID must be hexadecimal".to_string())
    })
}

pub(crate) fn parse_extended_pan_id(value: &str) -> Result<u64> {
    u64::from_str_radix(strip_hex_prefix(value), 16).map_err(|_| {
        OpenThreadError::InvalidConfiguration(
            "Thread extended PAN ID must be hexadecimal".to_string(),
        )
    })
}

pub(crate) fn parse_network_key(value: &str) -> Result<Vec<u8>> {
    decode_hex(strip_hex_prefix(value)).map_err(|_| {
        OpenThreadError::InvalidConfiguration(
            "Thread network key must be 32 hexadecimal characters".to_string(),
        )
    })
}

pub(crate) fn parse_imported_dataset(dataset_tlvs: &str) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = Zeroizing::new(parse_dataset_bytes(dataset_tlvs)?);
    let mut present = BTreeSet::new();
    let mut channel = None;
    let mut channel_mask = None;
    for tlv in parse_tlvs(&bytes)? {
        if !present.insert(tlv.kind) {
            return Err(OpenThreadError::InvalidDataset(format!(
                "the Active Operational Dataset contains duplicate TLV type {}",
                tlv.kind
            )));
        }
        validate_operational_tlv(tlv)?;
        match tlv.kind {
            CHANNEL_TLV => channel = Some(parse_dataset_channel(tlv.value)?),
            CHANNEL_MASK_TLV => channel_mask = Some(tlv.value),
            _ => {}
        }
    }
    let missing = REQUIRED_ACTIVE_DATASET_TLVS
        .into_iter()
        .filter(|kind| !present.contains(kind))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(OpenThreadError::InvalidDataset(format!(
            "the Active Operational Dataset is incomplete; missing TLV types {missing:?}"
        )));
    }
    let (page, channel) = channel.expect("required channel TLV was checked above");
    if !channel_mask.is_some_and(|mask| channel_mask_contains(mask, page, channel)) {
        return Err(OpenThreadError::InvalidDataset(format!(
            "the selected Thread channel {channel} is not enabled by the channel mask"
        )));
    }
    Ok(bytes)
}

fn validate_operational_tlv(tlv: DatasetTlv<'_>) -> Result<()> {
    let valid = match tlv.kind {
        CHANNEL_TLV => parse_dataset_channel(tlv.value).is_ok(),
        PAN_ID_TLV => tlv.value.len() == 2 && tlv.value != [0xff, 0xff],
        EXTENDED_PAN_ID_TLV => {
            tlv.value.len() == 8
                && tlv.value.iter().any(|byte| *byte != 0)
                && tlv.value.iter().any(|byte| *byte != 0xff)
        }
        NETWORK_NAME_TLV => valid_network_name(tlv.value),
        4 | 5 => tlv.value.len() == 16,
        MESH_LOCAL_PREFIX_TLV => tlv.value.len() == 8 && tlv.value[0] == 0xfd,
        SECURITY_POLICY_TLV => {
            (3..=4).contains(&tlv.value.len())
                && u16::from_be_bytes([tlv.value[0], tlv.value[1]]) != 0
        }
        14 => tlv.value.len() == 8,
        CHANNEL_MASK_TLV => valid_channel_mask(tlv.value),
        51 | 52 => false,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(OpenThreadError::InvalidDataset(format!(
            "TLV type {} has an invalid value for an Active Operational Dataset",
            tlv.kind
        )))
    }
}

fn valid_network_name(value: &[u8]) -> bool {
    let Ok(value) = std::str::from_utf8(value) else {
        return false;
    };
    !value.is_empty()
        && value.len() <= 16
        && value.is_ascii()
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn parse_dataset_channel(value: &[u8]) -> Result<(u8, u16)> {
    if value.len() != 3 {
        return Err(OpenThreadError::InvalidDataset(
            "the Thread channel TLV must contain a channel page and channel".to_string(),
        ));
    }
    let page = value[0];
    let channel = u16::from_be_bytes([value[1], value[2]]);
    if page != 0 || !(11..=26).contains(&channel) {
        return Err(OpenThreadError::InvalidDataset(
            "the active dataset must select an IEEE 802.15.4 channel from 11 to 26 on channel page 0"
                .to_string(),
        ));
    }
    Ok((page, channel))
}

fn channel_mask_contains(value: &[u8], selected_page: u8, channel: u16) -> bool {
    let mut cursor = 0;
    while cursor < value.len() {
        let page = value[cursor];
        let mask_length = usize::from(value[cursor + 1]);
        let mask = &value[cursor + 2..cursor + 2 + mask_length];
        if page == selected_page && mask.len() == 4 {
            let mask = u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]);
            let bit = u32::from(31 - channel);
            return mask & (1_u32 << bit) != 0;
        }
        cursor += 2 + mask_length;
    }
    false
}

fn valid_channel_mask(value: &[u8]) -> bool {
    let mut cursor = 0;
    let mut entries = 0;
    while cursor < value.len() {
        if value.len() - cursor < 2 {
            return false;
        }
        let mask_length = usize::from(value[cursor + 1]);
        if mask_length == 0 || cursor + 2 + mask_length > value.len() {
            return false;
        }
        cursor += 2 + mask_length;
        entries += 1;
    }
    entries > 0
}

pub(crate) fn parse_active_dataset_bytes(bytes: &[u8]) -> Result<ThreadActiveDataset> {
    if bytes.is_empty() {
        return Err(OpenThreadError::InvalidDataset(
            "the local Thread network has no Active Operational Dataset".to_string(),
        ));
    }
    if bytes.len() > MAX_OPERATIONAL_DATASET_BYTES {
        return Err(OpenThreadError::InvalidDataset(
            "the Active Operational Dataset exceeds OpenThread's maximum size".to_string(),
        ));
    }

    let mut network_key = None;
    let mut pskc = None;
    for tlv in parse_tlvs(bytes)? {
        match tlv.kind {
            NETWORK_KEY_TLV if tlv.value.len() == 16 => {
                network_key = Some(encode_hex(tlv.value));
            }
            PSKC_TLV if tlv.value.len() == 16 => pskc = Some(encode_hex(tlv.value)),
            _ => {}
        }
    }
    Ok(ThreadActiveDataset::new(
        encode_hex(bytes),
        network_key,
        pskc,
    ))
}

pub(crate) fn parse_mesh_local_prefix(bytes: &[u8]) -> Result<Option<[u8; 8]>> {
    for tlv in parse_tlvs(bytes)? {
        if tlv.kind != MESH_LOCAL_PREFIX_TLV {
            continue;
        }
        return tlv.value.try_into().map(Some).map_err(|_| {
            OpenThreadError::InvalidDataset(
                "the Active Operational Dataset contains an invalid mesh-local prefix".to_string(),
            )
        });
    }
    Ok(None)
}

#[cfg(test)]
pub(crate) fn parse_dataset_hex(dataset_tlvs: &str) -> Result<ThreadActiveDataset> {
    let bytes = parse_dataset_bytes(dataset_tlvs)?;
    parse_active_dataset_bytes(&bytes)
}

fn parse_dataset_bytes(dataset_tlvs: &str) -> Result<Vec<u8>> {
    let dataset_tlvs = dataset_tlvs.trim();
    if dataset_tlvs.is_empty() {
        return Err(OpenThreadError::InvalidDataset(
            "the dataset must not be empty".to_string(),
        ));
    }
    let bytes = decode_hex(dataset_tlvs).map_err(|_| {
        OpenThreadError::InvalidDataset(
            "the dataset must be an even-length hexadecimal value".to_string(),
        )
    })?;
    if bytes.len() > MAX_OPERATIONAL_DATASET_BYTES {
        return Err(OpenThreadError::InvalidDataset(
            "the dataset exceeds OpenThread's maximum size".to_string(),
        ));
    }
    parse_tlvs(&bytes)?;
    Ok(bytes)
}

#[derive(Clone, Copy)]
struct DatasetTlv<'a> {
    kind: u8,
    value: &'a [u8],
}

fn parse_tlvs(bytes: &[u8]) -> Result<Vec<DatasetTlv<'_>>> {
    let mut tlvs = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes.len() - cursor < 2 {
            return Err(OpenThreadError::InvalidDataset(
                "the dataset contains a truncated TLV header".to_string(),
            ));
        }
        let kind = bytes[cursor];
        let length = usize::from(bytes[cursor + 1]);
        let value_start = cursor + 2;
        let value_end = value_start + length;
        if value_end > bytes.len() {
            return Err(OpenThreadError::InvalidDataset(format!(
                "TLV type {kind} is truncated"
            )));
        }
        tlvs.push(DatasetTlv {
            kind,
            value: &bytes[value_start..value_end],
        });
        cursor = value_end;
    }
    Ok(tlvs)
}

fn normalize_optional_hex(
    label: &str,
    value: Option<String>,
    length: usize,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    normalize_hex(label, &value, length).map(Some)
}

fn normalize_optional_secret_hex(
    label: &str,
    value: Option<String>,
    length: usize,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = Zeroizing::new(value);
    normalize_hex(label, &value, length).map(Some)
}

fn normalize_hex(label: &str, value: &str, length: usize) -> Result<String> {
    let value = strip_hex_prefix(value);
    if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OpenThreadError::InvalidConfiguration(format!(
            "{label} must be {length} hexadecimal characters"
        )));
    }
    Ok(value.to_ascii_lowercase())
}

fn strip_hex_prefix(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

fn decode_hex(value: &str) -> std::result::Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| ())?;
            u8::from_str_radix(text, 16).map_err(|_| ())
        })
        .collect()
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CreateNetwork, DEFAULT_DEVELOPMENT_DATASET_TLVS};

    #[test]
    fn bundled_dataset_is_complete_and_extracts_credentials() {
        let bytes = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        let dataset = parse_active_dataset_bytes(&bytes).unwrap();
        assert_eq!(
            dataset.expose_network_key(),
            Some("b7fd07e5ce003cbe047f62c0b8bd7349")
        );
        assert_eq!(
            dataset.expose_pskc(),
            Some("5e9b9b360f80b88be2603fb0135c8d65")
        );
        assert_eq!(
            parse_mesh_local_prefix(&bytes).unwrap(),
            Some([0xfd, 0x35, 0x34, 0x41, 0x33, 0xd1, 0xd7, 0x3e])
        );
    }

    #[test]
    fn mesh_local_prefix_extraction_rejects_malformed_dataset_tlvs() {
        let error = parse_mesh_local_prefix(&[MESH_LOCAL_PREFIX_TLV, 7, 0xfd, 0, 0, 0, 0, 0, 0])
            .unwrap_err();
        assert!(error.to_string().contains("invalid mesh-local prefix"));
    }

    #[test]
    fn rejects_truncated_and_incomplete_datasets() {
        assert!(parse_dataset_hex("0510abcd").is_err());
        let error = parse_imported_dataset("051000112233445566778899aabbccddeeff")
            .expect_err("a network key alone is not a complete active dataset");
        assert!(error.to_string().contains("incomplete"));
    }

    #[test]
    fn rejects_duplicate_and_malformed_operational_parameters() {
        let duplicate_channel = format!("{DEFAULT_DEVELOPMENT_DATASET_TLVS}0003000019");
        let error = parse_imported_dataset(&duplicate_channel).unwrap_err();
        assert!(error.to_string().contains("duplicate TLV type 0"));

        let malformed_network_key = format!("0511{}", "00".repeat(17));
        let error = parse_imported_dataset(&malformed_network_key).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("TLV type 5 has an invalid value")
        );
    }

    #[test]
    fn rejects_semantically_inconsistent_active_datasets() {
        let masked_out_channel =
            DEFAULT_DEVELOPMENT_DATASET_TLVS.replace("35060004001fffe0", "3506000400000020");
        assert!(
            parse_imported_dataset(&masked_out_channel)
                .unwrap_err()
                .to_string()
                .contains("not enabled by the channel mask")
        );

        let reserved_pan = DEFAULT_DEVELOPMENT_DATASET_TLVS.replace("01027880", "0102ffff");
        assert!(parse_imported_dataset(&reserved_pan).is_err());

        let non_ula_prefix = DEFAULT_DEVELOPMENT_DATASET_TLVS.replacen("0708fd", "070820", 1);
        assert!(parse_imported_dataset(&non_ula_prefix).is_err());

        let padded_name = DEFAULT_DEVELOPMENT_DATASET_TLVS.replacen(
            "031065787472697474696f2d63362d646576",
            "031020787472697474696f2d63362d646576",
            1,
        );
        assert!(parse_imported_dataset(&padded_name).is_err());
    }

    #[test]
    fn create_network_normalizes_identifiers_and_rejects_padded_names() {
        let network = CreateNetwork::new(
            "Example".to_string(),
            Some(15),
            Some("0xABCD".to_string()),
            Some("0x0011223344556677".to_string()),
            Some("AABBCCDDEEFF00112233445566778899".to_string()),
        )
        .unwrap();
        assert_eq!(network.pan_id(), Some("abcd"));
        assert_eq!(network.extended_pan_id(), Some("0011223344556677"));
        assert!(CreateNetwork::new(" Example ".to_string(), None, None, None, None).is_err());
        assert!(
            CreateNetwork::new("                 X".to_string(), None, None, None, None).is_err()
        );
    }

    #[test]
    fn create_network_debug_redacts_the_key() {
        let network = CreateNetwork::new(
            "Example".to_string(),
            Some(15),
            None,
            None,
            Some("00112233445566778899aabbccddeeff".to_string()),
        )
        .unwrap();
        let debug = format!("{network:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("0011223344556677"));
    }
}
