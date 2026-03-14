import { useEffect, useState } from 'react';
import {
  Button,
  Callout,
  Dialog,
  DialogBody,
  DialogFooter,
  FormGroup,
  HTMLSelect,
  InputGroup,
  NumericInput,
  TextArea,
  Icon,
} from '@blueprintjs/core';
import { useRule, useCreateRule, useUpdateRule } from '../../hooks/use-rules';
import { useUIStore } from '../../stores/ui-store';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

interface ConditionRow {
  field: string;
  operator: string;
  value: string;
}

interface ActionRow {
  action_type: string;
  config: Record<string, unknown>;
}

const CONDITION_FIELDS = ['temperature', 'humidity', 'battery_level', 'status'];
const CONDITION_OPERATORS = ['>', '<', '>=', '<=', '==', '!='];
const ACTION_TYPES = ['alert', 'webhook', 'command'];
const SEVERITY_OPTIONS = ['info', 'warning', 'critical'];

const emptyCondition = (): ConditionRow => ({ field: 'temperature', operator: '>', value: '' });
const emptyAction = (): ActionRow => ({
  action_type: 'alert',
  config: { severity: 'warning' },
});

export function RuleDialog() {
  const { isRuleDialogOpen, editingRuleId, closeRuleDialog } = useUIStore();
  const { data: existingRule } = useRule(editingRuleId);
  const createMutation = useCreateRule();
  const updateMutation = useUpdateRule();

  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [triggerType, setTriggerType] = useState('telemetry');
  const [targetType, setTargetType] = useState('global');
  const [targetId, setTargetId] = useState('');
  const [cooldownSeconds, setCooldownSeconds] = useState(300);
  const [conditions, setConditions] = useState<ConditionRow[]>([emptyCondition()]);
  const [actions, setActions] = useState<ActionRow[]>([emptyAction()]);

  // Populate form when editing
  useEffect(() => {
    if (editingRuleId && existingRule) {
      setName(existingRule.name);
      setDescription(existingRule.description ?? '');
      setTriggerType(existingRule.trigger_type);
      setTargetType(existingRule.target_type);
      setTargetId(existingRule.target_id ?? '');
      setCooldownSeconds(existingRule.cooldown_seconds);
      setConditions(
        existingRule.conditions.length > 0
          ? existingRule.conditions.map((c) => ({
              field: c.field,
              operator: c.operator,
              value: c.value,
            }))
          : [emptyCondition()],
      );
      setActions(
        existingRule.actions.length > 0
          ? existingRule.actions.map((a) => ({
              action_type: a.action_type,
              config: { ...a.config },
            }))
          : [emptyAction()],
      );
    } else if (!editingRuleId) {
      resetForm();
    }
  }, [editingRuleId, existingRule]);

  const resetForm = () => {
    setName('');
    setDescription('');
    setTriggerType('telemetry');
    setTargetType('global');
    setTargetId('');
    setCooldownSeconds(300);
    setConditions([emptyCondition()]);
    setActions([emptyAction()]);
  };

  // Reset mutation errors when dialog opens/closes
  const resetCreate = createMutation.reset;
  const resetUpdate = updateMutation.reset;
  useEffect(() => {
    if (!isRuleDialogOpen) {
      resetCreate();
      resetUpdate();
    }
  }, [isRuleDialogOpen, resetCreate, resetUpdate]);

  const handleSubmit = () => {
    const body = {
      name,
      description: description || undefined,
      trigger_type: triggerType,
      target_type: targetType,
      target_id: targetType !== 'global' ? targetId || undefined : undefined,
      cooldown_seconds: cooldownSeconds,
      conditions: conditions.filter((c) => c.value.trim() !== ''),
      actions: actions.map((a) => ({ action_type: a.action_type, config: a.config })),
    };

    if (editingRuleId) {
      updateMutation.mutate(
        { id: editingRuleId, body },
        {
          onSuccess: () => {
            void showSuccessToast('Rule updated');
            closeRuleDialog();
            resetForm();
          },
          onError: () => {
            void showErrorToast('Failed to update rule');
          },
        },
      );
    } else {
      createMutation.mutate(body, {
        onSuccess: () => {
          void showSuccessToast('Rule created');
          closeRuleDialog();
          resetForm();
        },
        onError: () => {
          void showErrorToast('Failed to create rule');
        },
      });
    }
  };

  const updateCondition = (index: number, field: keyof ConditionRow, value: string) => {
    setConditions((prev) => prev.map((c, i) => (i === index ? { ...c, [field]: value } : c)));
  };

  const addCondition = () => setConditions((prev) => [...prev, emptyCondition()]);

  const removeCondition = (index: number) => {
    setConditions((prev) => (prev.length > 1 ? prev.filter((_, i) => i !== index) : prev));
  };

  const updateAction = (index: number, updates: Partial<ActionRow>) => {
    setActions((prev) => prev.map((a, i) => (i === index ? { ...a, ...updates } : a)));
  };

  const changeActionType = (index: number, newType: string) => {
    const defaultConfig: Record<string, unknown> =
      newType === 'alert'
        ? { severity: 'warning' }
        : newType === 'webhook'
          ? { url: '' }
          : { command: '' };
    updateAction(index, { action_type: newType, config: defaultConfig });
  };

  const addAction = () => setActions((prev) => [...prev, emptyAction()]);

  const removeAction = (index: number) => {
    setActions((prev) => (prev.length > 1 ? prev.filter((_, i) => i !== index) : prev));
  };

  const isPending = createMutation.isPending || updateMutation.isPending;
  const isError = createMutation.isError || updateMutation.isError;

  return (
    <Dialog
      icon={editingRuleId ? 'edit' : 'add'}
      title={editingRuleId ? 'Edit Rule' : 'Add Rule'}
      isOpen={isRuleDialogOpen}
      onClose={closeRuleDialog}
      style={{ width: 600 }}
    >
      <DialogBody>
        <FormGroup label="Name" labelInfo="(required)">
          <InputGroup
            placeholder="e.g. High Temperature Alert"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </FormGroup>

        <FormGroup label="Description">
          <TextArea
            fill
            placeholder="Optional description..."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={2}
          />
        </FormGroup>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
          <FormGroup label="Trigger Type">
            <HTMLSelect
              fill
              value={triggerType}
              onChange={(e) => setTriggerType(e.target.value)}
            >
              <option value="telemetry">Telemetry</option>
              <option value="device_status">Device Status</option>
            </HTMLSelect>
          </FormGroup>

          <FormGroup label="Target Type">
            <HTMLSelect
              fill
              value={targetType}
              onChange={(e) => setTargetType(e.target.value)}
            >
              <option value="global">Global</option>
              <option value="device_type">Device Type</option>
              <option value="fleet">Fleet</option>
              <option value="device">Device</option>
            </HTMLSelect>
          </FormGroup>
        </div>

        {targetType !== 'global' && (
          <FormGroup label="Target ID">
            <InputGroup
              placeholder={`Enter ${targetType} ID...`}
              value={targetId}
              onChange={(e) => setTargetId(e.target.value)}
            />
          </FormGroup>
        )}

        <FormGroup label="Cooldown (seconds)">
          <NumericInput
            fill
            min={0}
            value={cooldownSeconds}
            onValueChange={(val) => setCooldownSeconds(val)}
          />
        </FormGroup>

        {/* Conditions */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
            <span className="section-label" style={{ margin: 0 }}>Conditions</span>
            <Button icon="add" minimal small onClick={addCondition}>
              Add
            </Button>
          </div>
          {conditions.map((cond, i) => (
            <div key={i} style={{ display: 'flex', gap: 8, marginBottom: 8, alignItems: 'center' }}>
              <HTMLSelect
                value={cond.field}
                onChange={(e) => updateCondition(i, 'field', e.target.value)}
                style={{ flex: 1 }}
              >
                {CONDITION_FIELDS.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </HTMLSelect>
              <HTMLSelect
                value={cond.operator}
                onChange={(e) => updateCondition(i, 'operator', e.target.value)}
                style={{ width: 70 }}
              >
                {CONDITION_OPERATORS.map((op) => (
                  <option key={op} value={op}>
                    {op}
                  </option>
                ))}
              </HTMLSelect>
              <InputGroup
                placeholder="Value"
                value={cond.value}
                onChange={(e) => updateCondition(i, 'value', e.target.value)}
                style={{ flex: 1 }}
              />
              <Button
                icon="cross"
                minimal
                small
                disabled={conditions.length <= 1}
                onClick={() => removeCondition(i)}
              />
            </div>
          ))}
        </div>

        {/* Actions */}
        <div style={{ marginBottom: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
            <span className="section-label" style={{ margin: 0 }}>Actions</span>
            <Button icon="add" minimal small onClick={addAction}>
              Add
            </Button>
          </div>
          {actions.map((action, i) => (
            <div
              key={i}
              style={{
                padding: 12,
                marginBottom: 8,
                border: '1px solid var(--border-color)',
                background: 'hsla(0,0%,100%,0.02)',
              }}
            >
              <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8 }}>
                <HTMLSelect
                  value={action.action_type}
                  onChange={(e) => changeActionType(i, e.target.value)}
                  style={{ flex: 1 }}
                >
                  {ACTION_TYPES.map((t) => (
                    <option key={t} value={t}>
                      {t.charAt(0).toUpperCase() + t.slice(1)}
                    </option>
                  ))}
                </HTMLSelect>
                <Button
                  icon="cross"
                  minimal
                  small
                  disabled={actions.length <= 1}
                  onClick={() => removeAction(i)}
                />
              </div>
              {action.action_type === 'alert' && (
                <FormGroup label="Severity" style={{ marginBottom: 0 }}>
                  <HTMLSelect
                    fill
                    value={(action.config.severity as string) ?? 'warning'}
                    onChange={(e) =>
                      updateAction(i, { config: { ...action.config, severity: e.target.value } })
                    }
                  >
                    {SEVERITY_OPTIONS.map((s) => (
                      <option key={s} value={s}>
                        {s.charAt(0).toUpperCase() + s.slice(1)}
                      </option>
                    ))}
                  </HTMLSelect>
                </FormGroup>
              )}
              {action.action_type === 'webhook' && (
                <FormGroup label="URL" style={{ marginBottom: 0 }}>
                  <InputGroup
                    placeholder="https://..."
                    value={(action.config.url as string) ?? ''}
                    onChange={(e) =>
                      updateAction(i, { config: { ...action.config, url: e.target.value } })
                    }
                    leftIcon={<Icon icon="globe" />}
                  />
                </FormGroup>
              )}
              {action.action_type === 'command' && (
                <FormGroup label="Command" style={{ marginBottom: 0 }}>
                  <InputGroup
                    placeholder="Command name..."
                    value={(action.config.command as string) ?? ''}
                    onChange={(e) =>
                      updateAction(i, { config: { ...action.config, command: e.target.value } })
                    }
                    leftIcon={<Icon icon="console" />}
                  />
                </FormGroup>
              )}
            </div>
          ))}
        </div>

        {isError && (
          <Callout intent="danger" icon="error">
            Failed to save rule. Please try again.
          </Callout>
        )}
      </DialogBody>
      <DialogFooter
        actions={
          <>
            <Button onClick={closeRuleDialog}>Cancel</Button>
            <Button
              intent="primary"
              icon={editingRuleId ? 'tick' : 'add'}
              onClick={handleSubmit}
              loading={isPending}
              disabled={!name.trim()}
            >
              {editingRuleId ? 'Update Rule' : 'Add Rule'}
            </Button>
          </>
        }
      />
    </Dialog>
  );
}
