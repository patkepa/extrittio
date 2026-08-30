use std::collections::BTreeSet;

use serde_json::Value;

use crate::error::{ContractError, ValidationIssue};

/// The deliberately bounded JSON Schema vocabulary accepted by blueprint v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaProfile {
    ExtrittioV1,
}

const SUPPORTED_KEYWORDS: &[&str] = &[
    "$id",
    "$schema",
    "additionalProperties",
    "const",
    "default",
    "description",
    "enum",
    "examples",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "items",
    "maximum",
    "maxItems",
    "maxLength",
    "minimum",
    "minItems",
    "minLength",
    "properties",
    "required",
    "title",
    "type",
];

pub(crate) fn validate_schema_document(schema: &Value, path: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    inspect_schema(schema, path, 0, &mut issues);
    issues
}

fn inspect_schema(schema: &Value, path: &str, depth: usize, issues: &mut Vec<ValidationIssue>) {
    if depth > 32 {
        issues.push(ValidationIssue::new(
            path,
            "schema nesting exceeds 32 levels",
        ));
        return;
    }
    let Value::Object(object) = schema else {
        if !schema.is_boolean() {
            issues.push(ValidationIssue::new(
                path,
                "schema must be an object or boolean",
            ));
        }
        return;
    };

    for keyword in object.keys() {
        if !SUPPORTED_KEYWORDS.contains(&keyword.as_str()) {
            issues.push(ValidationIssue::new(
                format!("{path}/{keyword}"),
                "keyword is not supported by the extrittio-v1 schema profile",
            ));
        }
    }

    if let Some(value_type) = object.get("type") {
        let valid = value_type.as_str().is_some_and(|value| {
            matches!(
                value,
                "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
            )
        });
        if !valid {
            issues.push(ValidationIssue::new(
                format!("{path}/type"),
                "type must be one supported scalar type name",
            ));
        }
    }

    if let Some(properties) = object.get("properties") {
        if let Some(properties) = properties.as_object() {
            if properties.len() > 256 {
                issues.push(ValidationIssue::new(
                    format!("{path}/properties"),
                    "schema object exceeds 256 properties",
                ));
            }
            for (name, child) in properties {
                inspect_schema(
                    child,
                    &format!("{path}/properties/{}", escape_pointer(name)),
                    depth + 1,
                    issues,
                );
            }
        } else {
            issues.push(ValidationIssue::new(
                format!("{path}/properties"),
                "properties must be an object",
            ));
        }
    }

    if let Some(required) = object.get("required") {
        let Some(required) = required.as_array() else {
            issues.push(ValidationIssue::new(
                format!("{path}/required"),
                "required must be an array of unique strings",
            ));
            return;
        };
        let mut names = BTreeSet::new();
        for (index, name) in required.iter().enumerate() {
            let Some(name) = name.as_str() else {
                issues.push(ValidationIssue::new(
                    format!("{path}/required/{index}"),
                    "required entry must be a string",
                ));
                continue;
            };
            if !names.insert(name) {
                issues.push(ValidationIssue::new(
                    format!("{path}/required/{index}"),
                    "required entries must be unique",
                ));
            }
        }
    }

    if let Some(additional) = object.get("additionalProperties")
        && !additional.is_boolean()
    {
        inspect_schema(
            additional,
            &format!("{path}/additionalProperties"),
            depth + 1,
            issues,
        );
    }

    if let Some(items) = object.get("items") {
        inspect_schema(items, &format!("{path}/items"), depth + 1, issues);
    }

    for keyword in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if let Some(bound) = object.get(keyword)
            && !bound.is_number()
        {
            issues.push(ValidationIssue::new(
                format!("{path}/{keyword}"),
                format!("{keyword} must be numeric"),
            ));
        }
    }

    for keyword in ["minLength", "maxLength", "minItems", "maxItems"] {
        if let Some(bound) = object.get(keyword)
            && bound.as_u64().is_none()
        {
            issues.push(ValidationIssue::new(
                format!("{path}/{keyword}"),
                format!("{keyword} must be a non-negative integer"),
            ));
        }
    }

    if let Some(values) = object.get("enum")
        && values.as_array().is_none_or(Vec::is_empty)
    {
        issues.push(ValidationIssue::new(
            format!("{path}/enum"),
            "enum must be a non-empty array",
        ));
    }
}

/// Validate an instance using the bounded `extrittio-v1` schema profile.
pub fn validate_instance(
    _profile: SchemaProfile,
    schema: &Value,
    instance: &Value,
) -> Result<(), ContractError> {
    let schema_issues = validate_schema_document(schema, "$schema");
    if !schema_issues.is_empty() {
        return Err(ContractError::InvalidBlueprint(
            schema_issues.len(),
            schema_issues,
        ));
    }

    let mut issues = Vec::new();
    validate_node(schema, instance, "$", 0, &mut issues);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(ContractError::InvalidInstance(issues.len(), issues))
    }
}

fn validate_node(
    schema: &Value,
    instance: &Value,
    path: &str,
    depth: usize,
    issues: &mut Vec<ValidationIssue>,
) {
    if depth > 64 {
        issues.push(ValidationIssue::new(
            path,
            "instance nesting exceeds 64 levels",
        ));
        return;
    }
    if let Some(accept) = schema.as_bool() {
        if !accept {
            issues.push(ValidationIssue::new(path, "value is rejected by schema"));
        }
        return;
    }
    let Some(schema) = schema.as_object() else {
        return;
    };

    if let Some(expected) = schema.get("type").and_then(Value::as_str)
        && !matches_type(expected, instance)
    {
        issues.push(ValidationIssue::new(
            path,
            format!("expected {expected}, found {}", instance_kind(instance)),
        ));
        return;
    }

    if let Some(expected) = schema.get("const")
        && expected != instance
    {
        issues.push(ValidationIssue::new(path, "value does not equal const"));
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(instance)
    {
        issues.push(ValidationIssue::new(path, "value is not in enum"));
    }

    if let Some(number) = instance.as_f64() {
        check_numeric_bound(schema, "minimum", number, false, path, issues);
        check_numeric_bound(schema, "maximum", number, false, path, issues);
        check_numeric_bound(schema, "exclusiveMinimum", number, true, path, issues);
        check_numeric_bound(schema, "exclusiveMaximum", number, true, path, issues);
    }

    if let Some(string) = instance.as_str() {
        let length = string.chars().count() as u64;
        check_length(schema, "minLength", length, true, path, issues);
        check_length(schema, "maxLength", length, false, path, issues);
    }

    if let Some(array) = instance.as_array() {
        let length = array.len() as u64;
        check_length(schema, "minItems", length, true, path, issues);
        check_length(schema, "maxItems", length, false, path, issues);
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in array.iter().enumerate() {
                validate_node(
                    item_schema,
                    item,
                    &format!("{path}/{index}"),
                    depth + 1,
                    issues,
                );
            }
        }
    }

    if let Some(object) = instance.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for name in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(name) {
                    issues.push(ValidationIssue::new(
                        format!("{path}/{}", escape_pointer(name)),
                        "required property is missing",
                    ));
                }
            }
        }

        let properties = schema.get("properties").and_then(Value::as_object);
        for (name, value) in object {
            let property_path = format!("{path}/{}", escape_pointer(name));
            if let Some(property_schema) = properties.and_then(|values| values.get(name)) {
                validate_node(property_schema, value, &property_path, depth + 1, issues);
            } else if let Some(additional) = schema.get("additionalProperties") {
                if additional == &Value::Bool(false) {
                    issues.push(ValidationIssue::new(
                        property_path,
                        "additional property is not allowed",
                    ));
                } else if additional.is_object() {
                    validate_node(additional, value, &property_path, depth + 1, issues);
                }
            }
        }
    }
}

fn matches_type(expected: &str, value: &Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn instance_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn check_numeric_bound(
    schema: &serde_json::Map<String, Value>,
    keyword: &str,
    number: f64,
    exclusive: bool,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(bound) = schema.get(keyword).and_then(Value::as_f64) else {
        return;
    };
    let violates = match keyword {
        "minimum" | "exclusiveMinimum" => {
            if exclusive {
                number <= bound
            } else {
                number < bound
            }
        }
        _ => {
            if exclusive {
                number >= bound
            } else {
                number > bound
            }
        }
    };
    if violates {
        issues.push(ValidationIssue::new(
            path,
            format!("numeric value violates {keyword} {bound}"),
        ));
    }
}

fn check_length(
    schema: &serde_json::Map<String, Value>,
    keyword: &str,
    length: u64,
    minimum: bool,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(bound) = schema.get(keyword).and_then(Value::as_u64) else {
        return;
    };
    if (minimum && length < bound) || (!minimum && length > bound) {
        issues.push(ValidationIssue::new(
            path,
            format!("length {length} violates {keyword} {bound}"),
        ));
    }
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn validates_supported_object_schema() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["interval"],
            "properties": {
                "interval": {"type": "integer", "minimum": 5, "maximum": 60}
            }
        });

        validate_instance(
            SchemaProfile::ExtrittioV1,
            &schema,
            &json!({"interval": 30}),
        )
        .unwrap();
        let error = validate_instance(
            SchemaProfile::ExtrittioV1,
            &schema,
            &json!({"interval": 2, "unexpected": true}),
        )
        .unwrap_err();
        assert_eq!(error.validation_issues().unwrap().len(), 2);
    }

    #[test]
    fn rejects_unsupported_keywords() {
        let schema = json!({"type": "string", "pattern": "^[a-z]+$"});
        let error =
            validate_instance(SchemaProfile::ExtrittioV1, &schema, &json!("ok")).unwrap_err();
        assert!(matches!(error, ContractError::InvalidBlueprint(_, _)));
    }
}
