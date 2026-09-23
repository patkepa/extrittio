import type { Rule, CreateRuleRequest } from '../../../types/rules';

export interface ConditionRow {
  field: string;
  operator: string;
  value: string;
  zone_id?: string;
  blueprint_id?: string;
  blueprint_revision_id?: string;
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
  selectorBlueprintId: string;
  cooldownSeconds: number;
  conditions: ConditionRow[];
  actions: ActionRow[];
}

export function createRuleForm(rule?: Rule): RuleForm {
  const triggerType = rule?.trigger_type ?? 'telemetry';
  const metricSelector = rule?.conditions.find(
    (condition) => condition.selector.kind === 'metric',
  )?.selector;
  return {
    name: rule?.name ?? '',
    description: rule?.description ?? '',
    triggerType,
    targetType: rule?.target_type ?? 'global',
    targetId: rule?.target_id ?? '',
    selectorBlueprintId: metricSelector?.kind === 'metric' ? metricSelector.blueprint_id : '',
    cooldownSeconds: rule?.cooldown_seconds ?? 300,
    conditions: rule?.conditions.length
      ? rule.conditions.map(({ selector, operator, value }) => ({
          field:
            selector.kind === 'metric'
              ? `${selector.stream_key}.${selector.field_path}`
              : selector.kind === 'status'
                ? 'status'
                : selector.field,
          operator,
          value,
          zone_id: selector.kind === 'geofence' ? selector.zone_id : undefined,
          blueprint_id: selector.kind === 'metric' ? selector.blueprint_id : undefined,
          blueprint_revision_id:
            selector.kind === 'metric' ? selector.blueprint_revision_id : undefined,
        }))
      : [emptyCondition(triggerType)],
    actions: rule?.actions.length
      ? rule.actions.map(({ action_type, config }) => ({ action_type, config: { ...config } }))
      : [emptyAction()],
  };
}

const ruleTargets = new Set(['global', 'blueprint', 'fleet', 'device']);

function isRuleTarget(value: string): value is Rule['target_type'] {
  return ruleTargets.has(value);
}

export function validateRuleForm(form: RuleForm) {
  const zoneStates = form.conditions.filter((condition) => condition.field === 'zone_state');
  const dwells = form.conditions.filter((condition) => condition.field === 'dwell_seconds');
  const invalidGeofence =
    form.triggerType === 'geofence' &&
    (zoneStates.length !== 1 ||
      dwells.length > 1 ||
      form.conditions.length !== zoneStates.length + dwells.length ||
      form.conditions.some((condition) => condition.zone_id !== zoneStates[0]?.zone_id) ||
      (dwells.length > 0 && zoneStates[0]?.value !== 'inside') ||
      dwells.some((condition) => condition.operator !== 'gte' || !/^\d+$/.test(condition.value)));
  return {
    hasEmptyConditions:
      invalidGeofence ||
      form.conditions.some(
        (c) =>
          (form.triggerType === 'geofence' && !c.zone_id) ||
          c.field.trim() === '' ||
          c.value.trim() === '' ||
          (form.triggerType === 'telemetry' &&
            (!c.blueprint_id || !c.blueprint_revision_id || !/^.+\.\//.test(c.field))),
      ),
    hasInvalidActions: form.actions.some((a) => {
      if (a.action_type === 'webhook') {
        const url = (a.config.url as string) ?? '';
        return !url.trim() || !url.startsWith('https://');
      }
      return a.action_type === 'command' && !((a.config.command as string) ?? '').trim();
    }),
    hasMissingTarget:
      !ruleTargets.has(form.targetType) ||
      (form.targetType !== 'global' && form.targetId.trim() === ''),
  };
}

export function ruleFormRequest(form: RuleForm): CreateRuleRequest {
  if (!isRuleTarget(form.targetType)) throw new Error('Unsupported rule target');
  return {
    name: form.name,
    description: form.description || undefined,
    trigger_type: form.triggerType,
    target_type: form.targetType,
    target_id: form.targetType !== 'global' ? form.targetId || undefined : undefined,
    cooldown_seconds: form.cooldownSeconds,
    conditions: form.conditions.map((c) => ({
      selector:
        form.triggerType === 'telemetry'
          ? (() => {
              const separator = c.field.indexOf('./');
              return {
                kind: 'metric' as const,
                blueprint_id: c.blueprint_id ?? '',
                blueprint_revision_id: c.blueprint_revision_id ?? '',
                stream_key: c.field.slice(0, separator),
                field_path: c.field.slice(separator + 1),
              };
            })()
          : form.triggerType === 'geofence'
            ? { kind: 'geofence' as const, zone_id: c.zone_id ?? '', field: c.field }
            : { kind: 'status' as const },
      operator: c.operator,
      value: c.value,
    })),
    actions: form.actions.map((a) => ({ action_type: a.action_type, config: a.config })),
  };
}
