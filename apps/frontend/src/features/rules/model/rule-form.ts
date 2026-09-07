import type { Rule, CreateRuleRequest } from '../../../types/rules';

export interface ConditionRow {
  field: string;
  operator: string;
  value: string;
  zone_id?: string;
}

export interface ActionRow {
  action_type: string;
  config: Record<string, unknown>;
}

export const emptyCondition = (triggerType: string): ConditionRow => ({
  field:
    triggerType === 'device_status' ? 'status' : triggerType === 'geofence' ? 'zone_state' : '',
  operator: triggerType === 'device_status' || triggerType === 'geofence' ? 'eq' : 'gt',
  value: '',
  zone_id: undefined,
});
export const emptyAction = (): ActionRow => ({
  action_type: 'alert',
  config: { severity: 'warning' },
});

export interface RuleForm {
  name: string;
  description: string;
  triggerType: string;
  targetType: string;
  targetId: string;
  cooldownSeconds: number;
  conditions: ConditionRow[];
  actions: ActionRow[];
}

export function createRuleForm(rule?: Rule): RuleForm {
  const triggerType = rule?.trigger_type ?? 'telemetry';
  return {
    name: rule?.name ?? '',
    description: rule?.description ?? '',
    triggerType,
    targetType: rule?.target_type ?? 'global',
    targetId: rule?.target_id ?? '',
    cooldownSeconds: rule?.cooldown_seconds ?? 300,
    conditions: rule?.conditions.length
      ? rule.conditions.map(({ field, operator, value, zone_id }) => ({
          field,
          operator,
          value,
          zone_id,
        }))
      : [emptyCondition(triggerType)],
    actions: rule?.actions.length
      ? rule.actions.map(({ action_type, config }) => ({ action_type, config: { ...config } }))
      : [emptyAction()],
  };
}

export function validateRuleForm(form: RuleForm) {
  return {
    hasEmptyConditions: form.conditions.some(
      (c) =>
        (form.triggerType === 'geofence' && !c.zone_id) ||
        c.field.trim() === '' ||
        c.value.trim() === '',
    ),
    hasInvalidActions: form.actions.some((a) => {
      if (a.action_type === 'webhook') {
        const url = (a.config.url as string) ?? '';
        return !url.trim() || !url.startsWith('https://');
      }
      return a.action_type === 'command' && !((a.config.command as string) ?? '').trim();
    }),
    hasMissingTarget: form.targetType !== 'global' && form.targetId.trim() === '',
  };
}

export function ruleFormRequest(form: RuleForm): CreateRuleRequest {
  return {
    name: form.name,
    description: form.description || undefined,
    trigger_type: form.triggerType,
    target_type: form.targetType,
    target_id: form.targetType !== 'global' ? form.targetId || undefined : undefined,
    cooldown_seconds: form.cooldownSeconds,
    conditions: form.conditions.map((c) => ({
      field: c.field,
      operator: c.operator,
      value: c.value,
      ...(c.zone_id && { zone_id: c.zone_id }),
    })),
    actions: form.actions.map((a) => ({ action_type: a.action_type, config: a.config })),
  };
}
