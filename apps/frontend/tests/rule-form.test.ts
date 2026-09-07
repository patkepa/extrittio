import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createRuleForm,
  ruleFormRequest,
  validateRuleForm,
} from '../src/features/rules/model/rule-form.ts';
import type { Rule } from '../src/types/rules.ts';

const rule: Rule = {
  id: 'rule-1',
  name: 'Existing',
  description: null,
  enabled: true,
  trigger_type: 'geofence',
  target_type: 'device',
  target_id: 'device-1',
  cooldown_seconds: 0,
  conditions: [
    { id: 'condition-1', field: 'zone_state', operator: 'eq', value: 'inside', zone_id: 'zone-1' },
  ],
  actions: [
    { id: 'action-1', action_type: 'command', config: { command: 'restart', extra: 'preserved' } },
  ],
  created_at: '',
  updated_at: '',
};
test('editing drafts preserve stored values and do not mutate query data', () => {
  const form = createRuleForm(rule);
  assert.equal(form.cooldownSeconds, 0);
  form.conditions[0].value = 'outside';
  form.actions[0].config.command = 'calibrate';
  assert.equal(rule.conditions[0].value, 'inside');
  assert.equal(rule.actions[0].config.command, 'restart');
  assert.deepEqual(ruleFormRequest(form), {
    name: 'Existing',
    description: undefined,
    trigger_type: 'geofence',
    target_type: 'device',
    target_id: 'device-1',
    cooldown_seconds: 0,
    conditions: [{ field: 'zone_state', operator: 'eq', value: 'outside', zone_id: 'zone-1' }],
    actions: [{ action_type: 'command', config: { command: 'calibrate', extra: 'preserved' } }],
  });
});
test('each editing session starts from its own snapshot', () => {
  const draft = createRuleForm(rule);
  const refreshed = { ...rule, name: 'Server update' };
  assert.equal(draft.name, 'Existing');
  assert.equal(createRuleForm(refreshed).name, 'Server update');
  const empty = createRuleForm();
  assert.equal(empty.triggerType, 'telemetry');
  assert.equal(empty.conditions.length, 1);
  assert.equal(empty.actions.length, 1);
  empty.targetId = 'old-device';
  assert.equal(ruleFormRequest(empty).target_id, undefined);
});
test('validation retains geofence, target, webhook and command requirements', () => {
  const form = createRuleForm(rule);
  assert.deepEqual(validateRuleForm(form), {
    hasEmptyConditions: false,
    hasInvalidActions: false,
    hasMissingTarget: false,
  });
  form.conditions[0].zone_id = '';
  form.targetId = '';
  form.actions = [{ action_type: 'webhook', config: { url: 'http://example.com' } }];
  assert.deepEqual(validateRuleForm(form), {
    hasEmptyConditions: true,
    hasInvalidActions: true,
    hasMissingTarget: true,
  });
  form.actions[0].config.url = 'https://example.com';
  assert.equal(validateRuleForm(form).hasInvalidActions, false);
  form.actions = [{ action_type: 'command', config: { command: ' ' } }];
  assert.equal(validateRuleForm(form).hasInvalidActions, true);
});
