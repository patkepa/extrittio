use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{
    CommandDefinition, RouteDefinition, SchemaDefinition, StreamDefinition, TransportDefinition,
};
use crate::validation::ValidatedBlueprint;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub breaking: bool,
    pub changes: Vec<CompatibilityChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityChange {
    pub path: String,
    pub kind: CompatibilityChangeKind,
    pub breaking: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityChangeKind {
    Added,
    Removed,
    Changed,
}

/// Conservatively compare two valid published blueprint documents.
///
/// Schema changes are treated as breaking until a full schema-subsumption
/// checker exists; false-positive breaking classifications are safer than
/// deploying an incompatible device contract.
#[must_use]
pub fn compare_blueprints(
    previous: &ValidatedBlueprint,
    next: &ValidatedBlueprint,
) -> Compatibility {
    let previous = previous.blueprint();
    let next = next.blueprint();
    let mut changes = Vec::new();

    if next.spec.runtime.minimum_contract_api > previous.spec.runtime.minimum_contract_api {
        changed(
            &mut changes,
            "/spec/runtime/minimumContractApi",
            true,
            "minimum contract API increased",
        );
    }

    if next.spec.runtime.limits.max_message_bytes < previous.spec.runtime.limits.max_message_bytes
        || next.spec.runtime.limits.max_messages_per_minute
            < previous.spec.runtime.limits.max_messages_per_minute
        || next.spec.runtime.limits.max_metric_cardinality
            < previous.spec.runtime.limits.max_metric_cardinality
    {
        changed(
            &mut changes,
            "/spec/runtime/limits",
            true,
            "one or more runtime limits were reduced",
        );
    }

    compare_keyed(
        &previous.spec.transports,
        &next.spec.transports,
        |transport| transport.key.as_str(),
        "/spec/transports",
        true,
        &mut changes,
        compare_transport,
    );

    compare_keyed(
        &previous.spec.schemas,
        &next.spec.schemas,
        |schema| schema.key.as_str(),
        "/spec/schemas",
        true,
        &mut changes,
        compare_schema,
    );
    compare_keyed(
        &previous.spec.routes,
        &next.spec.routes,
        |route| route.key.as_str(),
        "/spec/routes",
        true,
        &mut changes,
        compare_route,
    );
    compare_keyed(
        &previous.spec.streams,
        &next.spec.streams,
        |stream| stream.key.as_str(),
        "/spec/streams",
        true,
        &mut changes,
        compare_stream,
    );
    compare_keyed(
        &previous.spec.commands,
        &next.spec.commands,
        |command| command.key.as_str(),
        "/spec/commands",
        true,
        &mut changes,
        compare_command,
    );

    compare_serialized(
        &previous.metadata,
        &next.metadata,
        "/metadata",
        false,
        &mut changes,
    );
    compare_serialized(
        &previous
            .spec
            .configuration
            .as_ref()
            .map(|value| &value.schema),
        &next.spec.configuration.as_ref().map(|value| &value.schema),
        "/spec/configuration/schema",
        true,
        &mut changes,
    );
    compare_serialized(
        &previous.spec.reported_state,
        &next.spec.reported_state,
        "/spec/reportedState",
        true,
        &mut changes,
    );
    compare_serialized(
        &previous.spec.presentation,
        &next.spec.presentation,
        "/spec/presentation",
        false,
        &mut changes,
    );

    Compatibility {
        breaking: changes.iter().any(|change| change.breaking),
        changes,
    }
}

fn compare_transport(
    previous: &TransportDefinition,
    next: &TransportDefinition,
    path: &str,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous.protocol != next.protocol || previous.binding != next.binding {
        changed(
            changes,
            path,
            true,
            "transport protocol or deployment binding changed",
        );
    } else if previous != next {
        changed(changes, path, false, "transport delivery policy changed");
    }
}

fn compare_schema(
    previous: &SchemaDefinition,
    next: &SchemaDefinition,
    path: &str,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous != next {
        changed(
            changes,
            path,
            true,
            "schema changed; schema changes are conservatively classified as breaking",
        );
    }
}

fn compare_route(
    previous: &RouteDefinition,
    next: &RouteDefinition,
    path: &str,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous != next {
        changed(changes, path, true, "route contract changed");
    }
}

fn compare_stream(
    previous: &StreamDefinition,
    next: &StreamDefinition,
    path: &str,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous.route != next.route || previous.timestamp != next.timestamp {
        changed(
            changes,
            path,
            true,
            "stream route or timestamp source changed",
        );
    }
    compare_keyed(
        &previous.fields,
        &next.fields,
        |field| field.path.as_str(),
        &format!("{path}/fields"),
        true,
        changes,
        |previous, next, field_path, changes| {
            if previous.value_type != next.value_type
                || previous.unit != next.unit
                || previous.semantic != next.semantic
            {
                changed(
                    changes,
                    field_path,
                    true,
                    "field type, unit, or semantic changed",
                );
            } else if previous != next {
                changed(changes, field_path, false, "field metadata changed");
            }
        },
    );
}

fn compare_command(
    previous: &CommandDefinition,
    next: &CommandDefinition,
    path: &str,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous.input_schema != next.input_schema || previous.result_schema != next.result_schema {
        changed(
            changes,
            path,
            true,
            "command input or result schema changed",
        );
    } else if previous != next {
        changed(changes, path, false, "command metadata or policy changed");
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_keyed<'a, T>(
    previous: &'a [T],
    next: &'a [T],
    key: impl Fn(&'a T) -> &'a str,
    path: &str,
    removal_breaking: bool,
    changes: &mut Vec<CompatibilityChange>,
    compare: impl Fn(&T, &T, &str, &mut Vec<CompatibilityChange>),
) {
    let previous = previous
        .iter()
        .map(|item| (key(item), item))
        .collect::<BTreeMap<_, _>>();
    let next = next
        .iter()
        .map(|item| (key(item), item))
        .collect::<BTreeMap<_, _>>();
    for (item_key, previous_item) in &previous {
        let item_path = format!("{path}/{item_key}");
        if let Some(next_item) = next.get(item_key) {
            compare(previous_item, next_item, &item_path, changes);
        } else {
            changes.push(CompatibilityChange {
                path: item_path,
                kind: CompatibilityChangeKind::Removed,
                breaking: removal_breaking,
                message: "declaration was removed".to_string(),
            });
        }
    }
    for item_key in next.keys() {
        if !previous.contains_key(item_key) {
            changes.push(CompatibilityChange {
                path: format!("{path}/{item_key}"),
                kind: CompatibilityChangeKind::Added,
                breaking: false,
                message: "declaration was added".to_string(),
            });
        }
    }
}

fn compare_serialized<T: PartialEq>(
    previous: &T,
    next: &T,
    path: &str,
    breaking: bool,
    changes: &mut Vec<CompatibilityChange>,
) {
    if previous != next {
        changed(changes, path, breaking, "declaration changed");
    }
}

fn changed(changes: &mut Vec<CompatibilityChange>, path: &str, breaking: bool, message: &str) {
    changes.push(CompatibilityChange {
        path: path.to_string(),
        kind: CompatibilityChangeKind::Changed,
        breaking,
        message: message.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use crate::{DeviceBlueprint, validate_blueprint};

    use super::*;

    fn fixture() -> ValidatedBlueprint {
        validate_blueprint(
            serde_yaml::from_str::<DeviceBlueprint>(include_str!(
                "../tests/fixtures/cold-room.yaml"
            ))
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn additions_are_compatible_and_removals_break() {
        let previous = fixture();
        let mut next_document = previous.blueprint().clone();
        next_document.spec.commands.clear();
        let next = validate_blueprint(next_document).unwrap();
        let result = compare_blueprints(&previous, &next);
        assert!(result.breaking);
        assert!(result.changes.iter().any(|change| {
            change.kind == CompatibilityChangeKind::Removed && change.path.ends_with("identify")
        }));
    }
}
