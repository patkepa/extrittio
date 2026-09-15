use crate::outbox::{NewOutboxEventRecord, OutboxEventRecord};
use crate::rule_engine::types::PendingAction;
use sha2::{Digest, Sha256};

pub fn outbox_event_for_action(
    action: &PendingAction,
) -> Result<NewOutboxEventRecord, serde_json::Error> {
    if matches!(
        action,
        PendingAction::UpdateZoneEntry { .. } | PendingAction::UpdateCooldown { .. }
    ) {
        return Err(serde::ser::Error::custom(
            "runtime state is not a deliverable action",
        ));
    }
    Ok(NewOutboxEventRecord {
        id: uuid::Uuid::new_v4().to_string(),
        tenant_id: tenant_id_for_action(action).to_string(),
        event_type: event_type_for_action(action).to_string(),
        aggregate_type: aggregate_type_for_action(action).to_string(),
        aggregate_id: aggregate_id_for_action(action),
        idempotency_key: Some(idempotency_key_for_action(action)),
        payload: serde_json::json!({"version": 1, "action": serde_json::to_value(action)?}),
    })
}

fn tenant_id_for_action(action: &PendingAction) -> &str {
    match action {
        PendingAction::CreateAlert { tenant_id, .. }
        | PendingAction::UpdateAlertValue { tenant_id, .. }
        | PendingAction::ResolveAlert { tenant_id, .. }
        | PendingAction::SendWebhook { tenant_id, .. }
        | PendingAction::SendCommand { tenant_id, .. }
        | PendingAction::UpdateCooldown { tenant_id, .. }
        | PendingAction::UpdateZoneEntry { tenant_id, .. } => tenant_id,
    }
}

#[cfg(test)]
mod zone_delivery_tests {
    use super::*;

    fn event(action: &PendingAction) -> OutboxEventRecord {
        let now = chrono::Utc::now().naive_utc();
        OutboxEventRecord {
            claim_token: None,
            id: "event".into(),
            tenant_id: tenant_id_for_action(action).into(),
            event_type: event_type_for_action(action).into(),
            aggregate_type: aggregate_type_for_action(action).into(),
            aggregate_id: aggregate_id_for_action(action),
            payload: serde_json::json!({"version": 1, "action": action}),
            attempts: 0,
            max_attempts: 3,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn delivery_requires_a_versioned_envelope_and_matching_metadata() {
        let action = PendingAction::ResolveAlert {
            tenant_id: "tenant".into(),
            alert_id: "alert".into(),
        };
        let mut record = event(&action);
        assert!(decode_event(&record).is_ok());
        record.payload = serde_json::to_value(&action).unwrap();
        assert!(decode_event(&record).is_err());
        record.payload = serde_json::json!({"version": 2, "action": action});
        assert!(decode_event(&record).is_err());
        record.payload = serde_json::json!({"version": 1});
        assert!(decode_event(&record).is_err());
        record = event(&action);
        record.tenant_id = "other".into();
        assert!(decode_event(&record).is_err());
        record = event(&action);
        record.event_type = "rule.send_command".into();
        assert!(decode_event(&record).is_err());
    }

    #[test]
    fn cooldown_state_is_neither_enqueued_nor_decoded_for_delivery() {
        let action = PendingAction::UpdateCooldown {
            tenant_id: "tenant".into(),
            rule_id: "rule".into(),
            device_id: "device".into(),
            fired_at: chrono::Utc::now().naive_utc(),
        };
        assert!(outbox_event_for_action(&action).is_err());
        assert!(decode_event(&event(&action)).is_err());
    }

    #[test]
    fn zone_state_cannot_be_enqueued_for_later_delivery() {
        let action = PendingAction::UpdateZoneEntry {
            tenant_id: "tenant".into(),
            rule_id: "rule".into(),
            device_id: "device".into(),
            entered_at: None,
        };
        assert!(outbox_event_for_action(&action).is_err());
        assert!(decode_event(&event(&action)).is_err());
    }
}

fn event_type_for_action(action: &PendingAction) -> &'static str {
    match action {
        PendingAction::CreateAlert { .. } => "rule.create_alert",
        PendingAction::UpdateAlertValue { .. } => "rule.update_alert_value",
        PendingAction::ResolveAlert { .. } => "rule.resolve_alert",
        PendingAction::SendWebhook { .. } => "rule.send_webhook",
        PendingAction::SendCommand { .. } => "rule.send_command",
        PendingAction::UpdateCooldown { .. } => "rule.update_cooldown",
        PendingAction::UpdateZoneEntry { .. } => "rule.update_zone_entry",
    }
}

fn aggregate_type_for_action(action: &PendingAction) -> &'static str {
    match action {
        PendingAction::CreateAlert { .. }
        | PendingAction::UpdateAlertValue { .. }
        | PendingAction::ResolveAlert { .. } => "alert",
        PendingAction::SendWebhook { .. } => "webhook",
        PendingAction::SendCommand { .. } => "command",
        PendingAction::UpdateCooldown { .. } => "rule_cooldown",
        PendingAction::UpdateZoneEntry { .. } => "zone_entry",
    }
}

fn aggregate_id_for_action(action: &PendingAction) -> String {
    match action {
        PendingAction::CreateAlert {
            rule_id, device_id, ..
        }
        | PendingAction::UpdateCooldown {
            rule_id, device_id, ..
        }
        | PendingAction::UpdateZoneEntry {
            rule_id, device_id, ..
        } => format!("{rule_id}:{device_id}"),
        PendingAction::UpdateAlertValue { alert_id, .. }
        | PendingAction::ResolveAlert { alert_id, .. } => alert_id.clone(),
        PendingAction::SendWebhook { url, .. } => url.clone(),
        PendingAction::SendCommand {
            device_id, command, ..
        } => format!("{device_id}:{command}"),
    }
}

fn idempotency_key_for_action(action: &PendingAction) -> String {
    match action {
        PendingAction::CreateAlert {
            rule_id, device_id, ..
        } => format!("create-alert:{rule_id}:{device_id}"),
        PendingAction::UpdateAlertValue {
            alert_id,
            triggered_value,
            ..
        } => format!(
            "update-alert-value:{alert_id}:{}",
            stable_hash(triggered_value.as_bytes())
        ),
        PendingAction::ResolveAlert { alert_id, .. } => format!("resolve-alert:{alert_id}"),
        PendingAction::SendWebhook {
            url,
            headers,
            payload,
            ..
        } => format!(
            "webhook:{url}:{}:{}",
            stable_json_hash(headers),
            stable_json_hash(payload)
        ),
        PendingAction::SendCommand {
            device_id,
            command,
            params,
            ..
        } => format!(
            "send-command:{device_id}:{command}:{}",
            stable_json_hash(params)
        ),
        PendingAction::UpdateCooldown {
            rule_id, device_id, ..
        } => format!("update-cooldown:{rule_id}:{device_id}"),
        PendingAction::UpdateZoneEntry {
            rule_id, device_id, ..
        } => format!("update-zone-entry:{rule_id}:{device_id}"),
    }
}

fn stable_json_hash<T>(value: &T) -> String
where
    T: serde::Serialize,
{
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    stable_hash(&bytes)
}

fn stable_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

/// Decode the required versioned envelope for external rule actions.
pub fn decode_event(event: &OutboxEventRecord) -> Result<PendingAction, crate::ApplicationError> {
    if event
        .payload
        .get("version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(crate::ApplicationError::InvalidOperation(
            "rule-action envelope requires version 1".into(),
        ));
    }
    let payload = event.payload.get("action").cloned().ok_or_else(|| {
        crate::ApplicationError::InvalidOperation("rule-action envelope has no action".into())
    })?;
    let action: PendingAction = serde_json::from_value(payload).map_err(|error| {
        crate::ApplicationError::InvalidOperation(format!("invalid persisted rule action: {error}"))
    })?;
    if matches!(
        action,
        PendingAction::UpdateZoneEntry { .. } | PendingAction::UpdateCooldown { .. }
    ) {
        return Err(crate::ApplicationError::InvalidOperation(
            "runtime state is not a deliverable action".into(),
        ));
    }
    if tenant_id_for_action(&action) != event.tenant_id
        || event_type_for_action(&action) != event.event_type
    {
        return Err(crate::ApplicationError::InvalidOperation(
            "persisted rule-action metadata does not match its payload".into(),
        ));
    }
    Ok(action)
}

/// Concrete destination validation, HTTP transport, and trace injection stay in the host.
#[async_trait::async_trait]
pub trait WebhookSender: Send + Sync {
    async fn send(
        &self,
        url: &str,
        headers: &std::collections::HashMap<String, String>,
        payload: &serde_json::Value,
        delivery_id: &str,
    ) -> Result<(), String>;
}
