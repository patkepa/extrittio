use std::collections::{BTreeMap, BTreeSet};

use crate::error::{ContractError, ValidationIssue};
use crate::model::{
    AggregateKind, DeviceBlueprint, HealthSignalSource, PayloadEncoding, RouteDirection,
    SchemaFormat, TransportProtocol,
};
use crate::schema::validate_schema_document;
use crate::schema::{SchemaProfile, validate_instance};
use crate::{BLUEPRINT_API_VERSION, BLUEPRINT_KIND, CONTRACT_API_VERSION};

#[derive(Debug, Clone)]
pub struct ValidatedBlueprint(DeviceBlueprint);

impl ValidatedBlueprint {
    #[must_use]
    pub fn blueprint(&self) -> &DeviceBlueprint {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> DeviceBlueprint {
        self.0
    }

    pub fn canonical_document(&self) -> Result<Vec<u8>, ContractError> {
        crate::canonical::canonical_json(&self.0)
    }

    pub fn document_hash(&self) -> Result<crate::ContractHash, ContractError> {
        Ok(crate::ContractHash::digest(&self.canonical_document()?))
    }
}

pub fn validate_blueprint(blueprint: DeviceBlueprint) -> Result<ValidatedBlueprint, ContractError> {
    let mut issues = Vec::new();
    if blueprint.api_version != BLUEPRINT_API_VERSION {
        issues.push(ValidationIssue::new(
            "/apiVersion",
            format!("must equal '{BLUEPRINT_API_VERSION}'"),
        ));
    }
    if blueprint.kind != BLUEPRINT_KIND {
        issues.push(ValidationIssue::new(
            "/kind",
            format!("must equal '{BLUEPRINT_KIND}'"),
        ));
    }
    validate_key(&blueprint.metadata.key, "/metadata/key", &mut issues);
    if blueprint.metadata.name.trim().is_empty() || blueprint.metadata.name.len() > 128 {
        issues.push(ValidationIssue::new(
            "/metadata/name",
            "name must contain 1-128 bytes",
        ));
    }
    if let Some(color) = &blueprint.metadata.color
        && !is_color(color)
    {
        issues.push(ValidationIssue::new(
            "/metadata/color",
            "color must be a #RRGGBB value",
        ));
    }

    let runtime = &blueprint.spec.runtime;
    if runtime.minimum_contract_api == 0 || runtime.minimum_contract_api > CONTRACT_API_VERSION {
        issues.push(ValidationIssue::new(
            "/spec/runtime/minimumContractApi",
            format!("must be between 1 and {CONTRACT_API_VERSION}"),
        ));
    }
    let heartbeat_ms = validate_duration(
        &runtime.heartbeat.interval,
        "/spec/runtime/heartbeat/interval",
        &mut issues,
    );
    let offline_ms = validate_duration(
        &runtime.heartbeat.offline_after,
        "/spec/runtime/heartbeat/offlineAfter",
        &mut issues,
    );
    if let (Some(heartbeat_ms), Some(offline_ms)) = (heartbeat_ms, offline_ms)
        && offline_ms <= heartbeat_ms
    {
        issues.push(ValidationIssue::new(
            "/spec/runtime/heartbeat/offlineAfter",
            "must be greater than the heartbeat interval",
        ));
    }
    if runtime.limits.max_message_bytes == 0 {
        issues.push(ValidationIssue::new(
            "/spec/runtime/limits/maxMessageBytes",
            "must be greater than zero",
        ));
    }
    if runtime.limits.max_messages_per_minute == 0 {
        issues.push(ValidationIssue::new(
            "/spec/runtime/limits/maxMessagesPerMinute",
            "must be greater than zero",
        ));
    }
    if runtime.limits.max_metric_cardinality == 0 {
        issues.push(ValidationIssue::new(
            "/spec/runtime/limits/maxMetricCardinality",
            "must be greater than zero",
        ));
    }

    let transports = keyed(
        &blueprint.spec.transports,
        |item| item.key.as_str(),
        "/spec/transports",
        &mut issues,
    );
    for (index, transport) in blueprint.spec.transports.iter().enumerate() {
        validate_key(
            &transport.key,
            &format!("/spec/transports/{index}/key"),
            &mut issues,
        );
        validate_key(
            &transport.binding,
            &format!("/spec/transports/{index}/binding"),
            &mut issues,
        );
    }

    let schemas = keyed(
        &blueprint.spec.schemas,
        |item| item.key.as_str(),
        "/spec/schemas",
        &mut issues,
    );
    for (index, schema) in blueprint.spec.schemas.iter().enumerate() {
        validate_schema_key(
            &schema.key,
            &format!("/spec/schemas/{index}/key"),
            &mut issues,
        );
        if schema.format == SchemaFormat::JsonSchema {
            issues.extend(validate_schema_document(
                &schema.schema,
                &format!("/spec/schemas/{index}/schema"),
            ));
        }
    }

    let routes = keyed(
        &blueprint.spec.routes,
        |item| item.key.as_str(),
        "/spec/routes",
        &mut issues,
    );
    for (index, route) in blueprint.spec.routes.iter().enumerate() {
        validate_key(
            &route.key,
            &format!("/spec/routes/{index}/key"),
            &mut issues,
        );
        if !transports.contains_key(route.transport.as_str()) {
            issues.push(ValidationIssue::new(
                format!("/spec/routes/{index}/transport"),
                format!("unknown transport '{}'", route.transport),
            ));
        }
        if !route.message_schema.starts_with("extrittio.")
            && !schemas.contains_key(route.message_schema.as_str())
        {
            issues.push(ValidationIssue::new(
                format!("/spec/routes/{index}/messageSchema"),
                format!("unknown schema '{}'", route.message_schema),
            ));
        }
        validate_address(&route.address, index, &mut issues);
        if transports
            .get(route.transport.as_str())
            .is_some_and(|transport| transport.protocol == TransportProtocol::Zenoh)
            && !route.address.starts_with("extrittio/devices/{device.id}/")
        {
            issues.push(ValidationIssue::new(
                format!("/spec/routes/{index}/address"),
                "Zenoh route addresses must start with 'extrittio/devices/{device.id}/'",
            ));
        }
        if let Some(schema) = schemas.get(route.message_schema.as_str()) {
            validate_encoding_schema(route.encoding, schema.format, index, &mut issues);
        }
    }

    let streams = keyed(
        &blueprint.spec.streams,
        |item| item.key.as_str(),
        "/spec/streams",
        &mut issues,
    );
    let mut metric_paths = BTreeSet::new();
    for (index, stream) in blueprint.spec.streams.iter().enumerate() {
        validate_key(
            &stream.key,
            &format!("/spec/streams/{index}/key"),
            &mut issues,
        );
        let route = routes.get(stream.route.as_str());
        if route.is_none() {
            issues.push(ValidationIssue::new(
                format!("/spec/streams/{index}/route"),
                format!("unknown route '{}'", stream.route),
            ));
        } else if route.is_some_and(|route| route.direction != RouteDirection::DeviceToCloud) {
            issues.push(ValidationIssue::new(
                format!("/spec/streams/{index}/route"),
                "stream route must have device_to_cloud direction",
            ));
        }
        let mut field_paths = BTreeSet::new();
        for (field_index, field) in stream.fields.iter().enumerate() {
            let path = format!("/spec/streams/{index}/fields/{field_index}");
            if !is_json_pointer(&field.path) {
                issues.push(ValidationIssue::new(
                    format!("{path}/path"),
                    "field path must be an absolute JSON pointer",
                ));
            }
            if !field_paths.insert(field.path.as_str()) {
                issues.push(ValidationIssue::new(
                    format!("{path}/path"),
                    "field path must be unique within its stream",
                ));
            }
            if field.label.trim().is_empty() {
                issues.push(ValidationIssue::new(
                    format!("{path}/label"),
                    "label must not be empty",
                ));
            }
            if !field.value_type.is_numeric()
                && field.aggregates.iter().any(|aggregate| {
                    !matches!(aggregate, AggregateKind::Count | AggregateKind::Last)
                })
            {
                issues.push(ValidationIssue::new(
                    format!("{path}/aggregates"),
                    "non-numeric fields support only count and last aggregates",
                ));
            }
            metric_paths.insert(format!("{}.{}", stream.key, field.path));
        }
    }

    keyed(
        &blueprint.spec.commands,
        |item| item.key.as_str(),
        "/spec/commands",
        &mut issues,
    );
    for (index, command) in blueprint.spec.commands.iter().enumerate() {
        validate_key(
            &command.key,
            &format!("/spec/commands/{index}/key"),
            &mut issues,
        );
        validate_duration(
            &command.timeout,
            &format!("/spec/commands/{index}/timeout"),
            &mut issues,
        );
        match routes.get(command.request_route.as_str()) {
            Some(route) if route.direction == RouteDirection::CloudToDevice => {}
            Some(_) => issues.push(ValidationIssue::new(
                format!("/spec/commands/{index}/requestRoute"),
                "command request route must be cloud_to_device",
            )),
            None => issues.push(ValidationIssue::new(
                format!("/spec/commands/{index}/requestRoute"),
                format!("unknown route '{}'", command.request_route),
            )),
        }
        match routes.get(command.response_route.as_str()) {
            Some(route) if route.direction == RouteDirection::DeviceToCloud => {}
            Some(_) => issues.push(ValidationIssue::new(
                format!("/spec/commands/{index}/responseRoute"),
                "command response route must be device_to_cloud",
            )),
            None => issues.push(ValidationIssue::new(
                format!("/spec/commands/{index}/responseRoute"),
                format!("unknown route '{}'", command.response_route),
            )),
        }
        issues.extend(validate_schema_document(
            &command.input_schema,
            &format!("/spec/commands/{index}/inputSchema"),
        ));
        issues.extend(validate_schema_document(
            &command.result_schema,
            &format!("/spec/commands/{index}/resultSchema"),
        ));
    }

    if let Some(configuration) = &blueprint.spec.configuration {
        let schema_issues =
            validate_schema_document(&configuration.schema, "/spec/configuration/schema");
        if schema_issues.is_empty()
            && !configuration.defaults.is_null()
            && let Err(error) = validate_instance(
                SchemaProfile::ExtrittioV1,
                &configuration.schema,
                &configuration.defaults,
            )
            && let Some(default_issues) = error.validation_issues()
        {
            issues.extend(default_issues.iter().map(|issue| {
                ValidationIssue::new(
                    format!(
                        "/spec/configuration/defaults{}",
                        issue.path.trim_start_matches('$')
                    ),
                    issue.message.clone(),
                )
            }));
        }
        issues.extend(schema_issues);
        validate_duration(
            &configuration.apply.acknowledgement_timeout,
            "/spec/configuration/apply/acknowledgementTimeout",
            &mut issues,
        );
    }
    if let Some(reported_state) = &blueprint.spec.reported_state {
        issues.extend(validate_schema_document(
            &reported_state.schema,
            "/spec/reportedState/schema",
        ));
    }

    if let Some(health) = &blueprint.spec.health {
        keyed(
            &health.signals,
            |item| item.key.as_str(),
            "/spec/health/signals",
            &mut issues,
        );
        for (index, signal) in health.signals.iter().enumerate() {
            validate_key(
                &signal.key,
                &format!("/spec/health/signals/{index}/key"),
                &mut issues,
            );
            if signal.source == HealthSignalSource::Metric
                && !metric_paths.contains(signal.path.as_str())
            {
                issues.push(ValidationIssue::new(
                    format!("/spec/health/signals/{index}/path"),
                    format!("unknown metric '{}'", signal.path),
                ));
            }
        }
    }

    for (index, relationship) in blueprint.spec.relationships.iter().enumerate() {
        validate_key(
            &relationship.key,
            &format!("/spec/relationships/{index}/key"),
            &mut issues,
        );
        if !streams.contains_key(relationship.stream.as_str()) {
            issues.push(ValidationIssue::new(
                format!("/spec/relationships/{index}/stream"),
                format!("unknown stream '{}'", relationship.stream),
            ));
        }
        if !is_json_pointer(&relationship.target_key_path) {
            issues.push(ValidationIssue::new(
                format!("/spec/relationships/{index}/targetKeyPath"),
                "target key path must be an absolute JSON pointer",
            ));
        }
    }

    if let Some(presentation) = &blueprint.spec.presentation {
        for (index, reference) in presentation.summary.iter().enumerate() {
            if !metric_paths.contains(reference.metric.as_str()) {
                issues.push(ValidationIssue::new(
                    format!("/spec/presentation/summary/{index}/metric"),
                    format!("unknown metric '{}'", reference.metric),
                ));
            }
        }
    }

    if blueprint
        .spec
        .streams
        .iter()
        .map(|stream| stream.fields.len())
        .sum::<usize>()
        > runtime.limits.max_metric_cardinality as usize
    {
        issues.push(ValidationIssue::new(
            "/spec/streams",
            "declared field count exceeds runtime maxMetricCardinality",
        ));
    }

    if issues.is_empty() {
        Ok(ValidatedBlueprint(blueprint))
    } else {
        Err(ContractError::InvalidBlueprint(issues.len(), issues))
    }
}

fn keyed<'a, T>(
    items: &'a [T],
    key: impl Fn(&'a T) -> &'a str,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) -> BTreeMap<&'a str, &'a T> {
    let mut values = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        let item_key = key(item);
        if values.insert(item_key, item).is_some() {
            issues.push(ValidationIssue::new(
                format!("{path}/{index}/key"),
                format!("duplicate key '{item_key}'"),
            ));
        }
    }
    values
}

fn validate_key(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    let valid = (1..=64).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        });
    if !valid {
        issues.push(ValidationIssue::new(
            path,
            "key must start with a lowercase letter and contain 1-64 lowercase letters, digits, '-', '_', or '.'",
        ));
    }
}

fn validate_schema_key(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    let Some((name, version)) = value.rsplit_once('@') else {
        issues.push(ValidationIssue::new(
            path,
            "schema key must end in '@<version>'",
        ));
        return;
    };
    validate_key(name, path, issues);
    if version
        .parse::<u32>()
        .ok()
        .is_none_or(|version| version == 0)
    {
        issues.push(ValidationIssue::new(
            path,
            "schema version must be a positive integer",
        ));
    }
}

fn validate_duration(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) -> Option<u64> {
    let duration = parse_duration_ms(value);
    if duration.is_none_or(|duration| duration == 0) {
        issues.push(ValidationIssue::new(
            path,
            "duration must be a positive integer followed by ms, s, m, or h",
        ));
    }
    duration
}

pub(crate) fn parse_duration_ms(value: &str) -> Option<u64> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600_000)
    } else {
        return None;
    };
    number.parse::<u64>().ok()?.checked_mul(multiplier)
}

fn validate_address(value: &str, index: usize, issues: &mut Vec<ValidationIssue>) {
    if value.is_empty() || value.len() > 512 {
        issues.push(ValidationIssue::new(
            format!("/spec/routes/{index}/address"),
            "address must contain 1-512 bytes",
        ));
    }
    let without_device = value.replace("{device.id}", "");
    if without_device.contains('{') || without_device.contains('}') {
        issues.push(ValidationIssue::new(
            format!("/spec/routes/{index}/address"),
            "only the {device.id} template variable is allowed",
        ));
    }
    if value.contains('*') || value.contains('#') {
        issues.push(ValidationIssue::new(
            format!("/spec/routes/{index}/address"),
            "wildcards are not allowed in device route addresses",
        ));
    }
}

fn validate_encoding_schema(
    encoding: PayloadEncoding,
    format: SchemaFormat,
    route_index: usize,
    issues: &mut Vec<ValidationIssue>,
) {
    let compatible = matches!(
        (encoding, format),
        (
            PayloadEncoding::Json | PayloadEncoding::Cbor,
            SchemaFormat::JsonSchema
        ) | (
            PayloadEncoding::ProtobufDynamic,
            SchemaFormat::ProtobufDescriptor
        ) | (PayloadEncoding::Raw, SchemaFormat::Opaque)
    );
    if !compatible {
        issues.push(ValidationIssue::new(
            format!("/spec/routes/{route_index}/encoding"),
            "encoding is incompatible with the referenced schema format",
        ));
    }
}

fn is_json_pointer(value: &str) -> bool {
    value.starts_with('/') && !value.contains("//")
}

fn is_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::parse_duration_ms;

    #[test]
    fn parses_bounded_duration_syntax() {
        assert_eq!(parse_duration_ms("250ms"), Some(250));
        assert_eq!(parse_duration_ms("30s"), Some(30_000));
        assert_eq!(parse_duration_ms("2m"), Some(120_000));
        assert_eq!(parse_duration_ms("1h"), Some(3_600_000));
        assert_eq!(parse_duration_ms("1.5s"), None);
    }
}
