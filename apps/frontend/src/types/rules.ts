export interface Rule {
  id: string;
  name: string;
  description: string | null;
  enabled: boolean;
  trigger_type: 'telemetry' | 'device_status' | 'geofence';
  target_type: 'global' | 'blueprint' | 'fleet' | 'device';
  target_id: string | null;
  cooldown_seconds: number;
  conditions: RuleCondition[];
  actions: RuleAction[];
  created_at: string;
  updated_at: string;
}

export interface RuleCondition {
  id: string;
  selector: RuleConditionSelector;
  operator: string;
  value: string;
}

export type RuleConditionSelector =
  | {
      kind: 'metric';
      blueprint_id: string;
      blueprint_revision_id: string;
      stream_key: string;
      field_path: string;
    }
  | { kind: 'status' }
  | { kind: 'geofence'; zone_id: string; field: string };

export interface RuleAction {
  id: string;
  action_type: 'alert' | 'webhook' | 'command';
  config: Record<string, unknown>;
}

export interface CreateRuleRequest {
  name: string;
  description?: string;
  trigger_type: string;
  target_type: Rule['target_type'];
  target_id?: string;
  cooldown_seconds?: number;
  conditions: { selector: RuleConditionSelector; operator: string; value: string }[];
  actions: { action_type: string; config: Record<string, unknown> }[];
}

export type UpdateRuleRequest = Partial<CreateRuleRequest>;
