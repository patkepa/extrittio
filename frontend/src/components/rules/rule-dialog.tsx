import { useCallback, useEffect, useState } from 'react';
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
import { useConfirmShortcut } from '../../lib/interactions';
import { useDeviceTypes } from '../../hooks/use-device-types';
import { useFleets } from '../../hooks/use-fleets';
import { useAllDevices } from '../../hooks/use-devices';
import { useZones } from '../../hooks/use-zones';
import { useUIStore } from '../../stores/ui-store';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

interface ConditionRow {
  field: string;
  operator: string;
  value: string;
  zone_id?: string;
}

interface ActionRow {
  action_type: string;
  config: Record<string, unknown>;
}

const TELEMETRY_FIELDS = [
  { value: 'temperature', label: 'Temperature' },
  { value: 'humidity', label: 'Humidity' },
  { value: 'battery_level', label: 'Battery Level' },
  { value: 'latitude', label: 'Latitude' },
  { value: 'longitude', label: 'Longitude' },
  { value: 'speed', label: 'Speed' },
  { value: 'altitude', label: 'Altitude' },
  { value: 'heading', label: 'Heading' },
];
const STATUS_FIELDS = [{ value: 'status', label: 'Status' }];

const GEOFENCE_FIELDS = [
  { label: 'Zone State', value: 'zone_state' },
  { label: 'Dwell Time (seconds)', value: 'dwell_seconds' },
];

const ZONE_STATE_VALUES = [
  { label: 'Inside', value: 'inside' },
  { label: 'Outside', value: 'outside' },
];

const NUMERIC_OPERATORS = [
  { value: 'gt', label: '>' },
  { value: 'gte', label: '>=' },
  { value: 'lt', label: '<' },
  { value: 'lte', label: '<=' },
  { value: 'eq', label: '=' },
  { value: 'neq', label: '!=' },
];
const STATUS_OPERATORS = [
  { value: 'eq', label: '=' },
  { value: 'neq', label: '!=' },
];
const ZONE_STATE_OPERATORS = [
  { value: 'eq', label: '=' },
  { value: 'neq', label: '!=' },
];
const STATUS_VALUES = ['online', 'offline', 'warning'];

const ACTION_TYPES = ['alert', 'webhook', 'command'];
const SEVERITY_OPTIONS = ['info', 'warning', 'critical'];

const emptyCondition = (triggerType: string): ConditionRow => ({
  field:
    triggerType === 'device_status'
      ? 'status'
      : triggerType === 'geofence'
        ? 'zone_state'
        : 'temperature',
  operator: triggerType === 'device_status' || triggerType === 'geofence' ? 'eq' : 'gt',
  value: '',
  zone_id: undefined,
});
const emptyAction = (): ActionRow => ({
  action_type: 'alert',
  config: { severity: 'warning' },
});

export function RuleDialog() {
  const { isRuleDialogOpen, editingRuleId, closeRuleDialog } = useUIStore();
  const { data: existingRule } = useRule(editingRuleId);
  const createMutation = useCreateRule();
  const updateMutation = useUpdateRule();

  // Data for target dropdowns
  const { data: deviceTypes } = useDeviceTypes();
  const { data: fleets } = useFleets();
  const { data: devicesData } = useAllDevices();
  const devices = devicesData?.data ?? [];
  const { data: zones = [] } = useZones();

  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [triggerType, setTriggerType] = useState('telemetry');
  const [targetType, setTargetType] = useState('global');
  const [targetId, setTargetId] = useState('');
  const [cooldownSeconds, setCooldownSeconds] = useState(300);
  const [conditions, setConditions] = useState<ConditionRow[]>([emptyCondition('telemetry')]);
  const [actions, setActions] = useState<ActionRow[]>([emptyAction()]);

  const resetForm = useCallback(() => {
    setName('');
    setDescription('');
    setTriggerType('telemetry');
    setTargetType('global');
    setTargetId('');
    setCooldownSeconds(300);
    setConditions([emptyCondition('telemetry')]);
    setActions([emptyAction()]);
  }, []);

  // Populate form when editing
  /* eslint-disable react-hooks/set-state-in-effect -- hydrate form state when async rule data arrives */
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
              zone_id: c.zone_id,
            }))
          : [emptyCondition(existingRule.trigger_type)],
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
  }, [editingRuleId, existingRule, resetForm]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Reset mutation errors when dialog opens/closes
  const resetCreate = createMutation.reset;
  const resetUpdate = updateMutation.reset;
  useEffect(() => {
    if (!isRuleDialogOpen) {
      resetCreate();
      resetUpdate();
    }
  }, [isRuleDialogOpen, resetCreate, resetUpdate]);

  const hasEmptyConditions = conditions.some((c) => {
    if (triggerType === 'geofence' && !c.zone_id) return true;
    return c.value.trim() === '';
  });
  const hasInvalidActions = actions.some((a) => {
    if (a.action_type === 'webhook') {
      const url = (a.config.url as string) ?? '';
      return !url.trim() || (!url.startsWith('http://') && !url.startsWith('https://'));
    }
    if (a.action_type === 'command') {
      return !((a.config.command as string) ?? '').trim();
    }
    return false;
  });

  const handleSubmit = () => {
    if (hasEmptyConditions) {
      void showErrorToast('All conditions must have a value');
      return;
    }
    if (hasInvalidActions) {
      void showErrorToast('Webhook actions require a valid URL, command actions require a name');
      return;
    }

    const body = {
      name,
      description: description || undefined,
      trigger_type: triggerType,
      target_type: targetType,
      target_id: targetType !== 'global' ? targetId || undefined : undefined,
      cooldown_seconds: cooldownSeconds,
      conditions: conditions.map((c) => ({
        field: c.field,
        operator: c.operator,
        value: c.value,
        ...(c.zone_id && { zone_id: c.zone_id }),
      })),
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

  const addCondition = () => setConditions((prev) => [...prev, emptyCondition(triggerType)]);

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
  const canSubmit = !!name.trim() && !hasEmptyConditions && !hasInvalidActions && !isPending;

  useConfirmShortcut({
    isOpen: isRuleDialogOpen,
    canConfirm: canSubmit,
    onConfirm: handleSubmit,
  });

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
              onChange={(e) => {
                const newType = e.target.value;
                setTriggerType(newType);
                setConditions([emptyCondition(newType)]);
              }}
            >
              <option value="telemetry">Telemetry</option>
              <option value="device_status">Device Status</option>
              <option value="geofence">Geofence</option>
            </HTMLSelect>
          </FormGroup>

          <FormGroup label="Target Type">
            <HTMLSelect
              fill
              value={targetType}
              onChange={(e) => {
                setTargetType(e.target.value);
                setTargetId('');
              }}
            >
              <option value="global">Global</option>
              <option value="device_type">Device Type</option>
              <option value="fleet">Fleet</option>
              <option value="device">Device</option>
            </HTMLSelect>
          </FormGroup>
        </div>

        {targetType === 'device_type' && (
          <FormGroup label="Device Type">
            <HTMLSelect fill value={targetId} onChange={(e) => setTargetId(e.target.value)}>
              <option value="">Select device type...</option>
              {(deviceTypes ?? []).map((dt) => (
                <option key={dt.id} value={String(dt.id)}>
                  {dt.name}
                </option>
              ))}
            </HTMLSelect>
          </FormGroup>
        )}
        {targetType === 'fleet' && (
          <FormGroup label="Fleet">
            <HTMLSelect fill value={targetId} onChange={(e) => setTargetId(e.target.value)}>
              <option value="">Select fleet...</option>
              {(fleets ?? []).map((f) => (
                <option key={f.id} value={String(f.id)}>
                  {f.name}
                </option>
              ))}
            </HTMLSelect>
          </FormGroup>
        )}
        {targetType === 'device' && (
          <FormGroup label="Device">
            <HTMLSelect fill value={targetId} onChange={(e) => setTargetId(e.target.value)}>
              <option value="">Select device...</option>
              {devices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name} ({d.device_type_name})
                </option>
              ))}
            </HTMLSelect>
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
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: 8,
            }}
          >
            <span className="section-label" style={{ margin: 0 }}>
              Conditions
            </span>
            <Button icon="add" minimal small onClick={addCondition}>
              Add
            </Button>
          </div>
          {conditions.map((cond, i) => {
            if (triggerType === 'geofence') {
              const isZoneState = cond.field === 'zone_state';
              const operators = isZoneState ? ZONE_STATE_OPERATORS : NUMERIC_OPERATORS;

              return (
                <div
                  key={i}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 6,
                    marginBottom: 12,
                    padding: 8,
                    border: '1px solid var(--border-color)',
                    background: 'hsla(0,0%,100%,0.02)',
                  }}
                >
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    {/* Field selector */}
                    <HTMLSelect
                      value={cond.field}
                      onChange={(e) => {
                        const newField = e.target.value;
                        updateCondition(i, 'field', newField);
                        // Reset operator and value when field changes
                        setConditions((prev) =>
                          prev.map((c, idx) =>
                            idx === i
                              ? {
                                  ...c,
                                  field: newField,
                                  operator: newField === 'zone_state' ? 'eq' : 'gt',
                                  value: '',
                                }
                              : c,
                          ),
                        );
                      }}
                      style={{ flex: 1 }}
                    >
                      {GEOFENCE_FIELDS.map((f) => (
                        <option key={f.value} value={f.value}>
                          {f.label}
                        </option>
                      ))}
                    </HTMLSelect>
                    <Button
                      icon="cross"
                      minimal
                      small
                      disabled={conditions.length <= 1}
                      onClick={() => removeCondition(i)}
                    />
                  </div>
                  {/* Zone picker */}
                  <HTMLSelect
                    value={cond.zone_id ?? ''}
                    onChange={(e) => updateCondition(i, 'zone_id', e.target.value)}
                  >
                    <option value="">Select zone...</option>
                    {zones.map((z) => (
                      <option key={z.id} value={z.id}>
                        {z.name}
                      </option>
                    ))}
                  </HTMLSelect>
                  {/* Operator + value row */}
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    <HTMLSelect
                      value={cond.operator}
                      onChange={(e) => updateCondition(i, 'operator', e.target.value)}
                      style={{ width: 70 }}
                    >
                      {operators.map((op) => (
                        <option key={op.value} value={op.value}>
                          {op.label}
                        </option>
                      ))}
                    </HTMLSelect>
                    {isZoneState ? (
                      <HTMLSelect
                        value={cond.value}
                        onChange={(e) => updateCondition(i, 'value', e.target.value)}
                        style={{ flex: 1 }}
                      >
                        <option value="">Select state...</option>
                        {ZONE_STATE_VALUES.map((sv) => (
                          <option key={sv.value} value={sv.value}>
                            {sv.label}
                          </option>
                        ))}
                      </HTMLSelect>
                    ) : (
                      <InputGroup
                        placeholder="Threshold seconds"
                        value={cond.value}
                        onChange={(e) => updateCondition(i, 'value', e.target.value)}
                        type="number"
                        style={{ flex: 1 }}
                      />
                    )}
                  </div>
                </div>
              );
            }

            // Telemetry / device_status conditions (original layout)
            const fields = triggerType === 'device_status' ? STATUS_FIELDS : TELEMETRY_FIELDS;
            const operators =
              triggerType === 'device_status' ? STATUS_OPERATORS : NUMERIC_OPERATORS;
            const isStatusField = cond.field === 'status';

            return (
              <div
                key={i}
                style={{ display: 'flex', gap: 8, marginBottom: 8, alignItems: 'center' }}
              >
                <HTMLSelect
                  value={cond.field}
                  onChange={(e) => updateCondition(i, 'field', e.target.value)}
                  style={{ flex: 1 }}
                >
                  {fields.map((f) => (
                    <option key={f.value} value={f.value}>
                      {f.label}
                    </option>
                  ))}
                </HTMLSelect>
                <HTMLSelect
                  value={cond.operator}
                  onChange={(e) => updateCondition(i, 'operator', e.target.value)}
                  style={{ width: 70 }}
                >
                  {operators.map((op) => (
                    <option key={op.value} value={op.value}>
                      {op.label}
                    </option>
                  ))}
                </HTMLSelect>
                {isStatusField ? (
                  <HTMLSelect
                    value={cond.value}
                    onChange={(e) => updateCondition(i, 'value', e.target.value)}
                    style={{ flex: 1 }}
                  >
                    <option value="">Select status...</option>
                    {STATUS_VALUES.map((s) => (
                      <option key={s} value={s}>
                        {s}
                      </option>
                    ))}
                  </HTMLSelect>
                ) : (
                  <InputGroup
                    placeholder="Threshold value"
                    value={cond.value}
                    onChange={(e) => updateCondition(i, 'value', e.target.value)}
                    type="number"
                    style={{ flex: 1 }}
                  />
                )}
                <Button
                  icon="cross"
                  minimal
                  small
                  disabled={conditions.length <= 1}
                  onClick={() => removeCondition(i)}
                />
              </div>
            );
          })}
        </div>

        {/* Actions */}
        <div style={{ marginBottom: 8 }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: 8,
            }}
          >
            <span className="section-label" style={{ margin: 0 }}>
              Actions
            </span>
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
              disabled={!canSubmit}
            >
              {editingRuleId ? 'Update Rule' : 'Add Rule'}
            </Button>
          </>
        }
      />
    </Dialog>
  );
}
