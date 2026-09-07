import { useMemo, useState, type SetStateAction } from 'react';
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
  Spinner,
} from '@blueprintjs/core';
import { useRule, useCreateRule, useUpdateRule } from '../queries/use-rules';
import { useConfirmShortcut } from '@patkepa/kantzen-ui/interactions';
import { useDeviceTypes } from '../../../hooks/use-device-types';
import { useFleets } from '../../../hooks/use-fleets';
import { useAllDevices } from '../../../hooks/use-devices';
import { useDeviceContract } from '../../../hooks/use-devices';
import {
  useDeviceBlueprints,
  useLatestDeviceBlueprintRevision,
} from '../../../hooks/use-device-blueprints';
import { useZones } from '../../../hooks/use-zones';
import { useUIStore } from '../../../stores/ui-store';
import { showSuccessToast, showErrorToast } from '../../../utils/toaster';
import {
  blueprintRuleCommands,
  blueprintRuleMetricFields,
  contractRuleCommands,
  contractRuleMetricFields,
} from '../model/rule-metric-fields';

import type { Rule } from '../../../types/rules';
import {
  createRuleForm,
  validateRuleForm,
  ruleFormRequest,
  emptyCondition,
  emptyAction,
  type RuleForm,
  type ConditionRow,
  type ActionRow,
} from '../model/rule-form';
import { RuleConditions } from './rule-conditions';
import { RuleActions } from './rule-actions';

export function RuleDialog() {
  const isOpen = useUIStore((state) => state.isRuleDialogOpen);
  const editingRuleId = useUIStore((state) => state.editingRuleId);
  const close = useUIStore((state) => state.closeRuleDialog);
  if (!isOpen) return null;
  return (
    <RuleDialogLoader
      key={editingRuleId ?? 'new'}
      editingRuleId={editingRuleId}
      closeRuleDialog={close}
    />
  );
}

interface EditorProps {
  editingRuleId: string | null;
  closeRuleDialog: () => void;
  existingRule?: Rule;
}

function RuleDialogLoader({ editingRuleId, closeRuleDialog }: EditorProps) {
  const query = useRule(editingRuleId);
  if (editingRuleId && !query.data) {
    return (
      <Dialog isOpen title="Edit Rule" onClose={closeRuleDialog}>
        <DialogBody>
          {query.isError ? (
            <Callout intent="danger">
              Failed to load rule. <Button onClick={() => void query.refetch()}>Retry</Button>
            </Callout>
          ) : (
            <Spinner aria-label="Loading rule" />
          )}
        </DialogBody>
      </Dialog>
    );
  }
  return (
    <RuleEditor
      editingRuleId={editingRuleId}
      closeRuleDialog={closeRuleDialog}
      existingRule={query.data}
    />
  );
}

function RuleEditor({ editingRuleId, closeRuleDialog, existingRule }: EditorProps) {
  const createMutation = useCreateRule();
  const updateMutation = useUpdateRule();
  const [form, setForm] = useState(() => createRuleForm(existingRule));
  const {
    name,
    description,
    triggerType,
    targetType,
    targetId,
    cooldownSeconds,
    conditions,
    actions,
  } = form;
  function setField<K extends keyof RuleForm>(field: K, value: SetStateAction<RuleForm[K]>) {
    setForm((previous) => ({
      ...previous,
      [field]:
        typeof value === 'function'
          ? (value as (previous: RuleForm[K]) => RuleForm[K])(previous[field])
          : value,
    }));
  }
  const setName = (value: string) => setField('name', value);
  const setDescription = (value: string) => setField('description', value);
  const setTriggerType = (value: string) => setField('triggerType', value);
  const setTargetType = (value: string) => setField('targetType', value);
  const setTargetId = (value: string) => setField('targetId', value);
  const setCooldownSeconds = (value: number) => setField('cooldownSeconds', value);
  const setConditions = (value: SetStateAction<ConditionRow[]>) => setField('conditions', value);
  const setActions = (value: SetStateAction<ActionRow[]>) => setField('actions', value);

  const { data: deviceTypes } = useDeviceTypes();
  const { data: fleets } = useFleets();
  const { data: devicesData } = useAllDevices(undefined, { enabled: targetType === 'device' });
  const devices = devicesData?.data ?? [];
  const { data: zones = [] } = useZones();
  const { data: blueprints = [] } = useDeviceBlueprints();
  const blueprintRevisionQuery = useLatestDeviceBlueprintRevision(
    targetType === 'blueprint' ? targetId : '',
  );
  const deviceContractQuery = useDeviceContract(targetType === 'device' ? targetId : '', {
    retry: false,
  });
  const telemetryFields = useMemo(() => {
    if (targetType === 'blueprint') {
      return blueprintRuleMetricFields(blueprintRevisionQuery.data);
    }
    if (targetType === 'device') {
      return contractRuleMetricFields(deviceContractQuery.data);
    }
    return [];
  }, [blueprintRevisionQuery.data, deviceContractQuery.data, targetType]);
  const commandOptions = useMemo(() => {
    if (targetType === 'blueprint') {
      return blueprintRuleCommands(blueprintRevisionQuery.data);
    }
    if (targetType === 'device') {
      return contractRuleCommands(deviceContractQuery.data);
    }
    return [];
  }, [blueprintRevisionQuery.data, deviceContractQuery.data, targetType]);

  const { hasEmptyConditions, hasInvalidActions, hasMissingTarget } = validateRuleForm(form);

  const handleSubmit = () => {
    if (hasMissingTarget) {
      void showErrorToast('Select a rule target');
      return;
    }
    if (hasEmptyConditions) {
      void showErrorToast('All conditions must have a value');
      return;
    }
    if (hasInvalidActions) {
      void showErrorToast('Webhook actions require a valid URL, command actions require a name');
      return;
    }

    const body = ruleFormRequest(form);

    if (editingRuleId) {
      updateMutation.mutate(
        { id: editingRuleId, body },
        {
          onSuccess: () => {
            void showSuccessToast('Rule updated');
            closeRuleDialog();
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
  const canSubmit =
    !!name.trim() && !hasMissingTarget && !hasEmptyConditions && !hasInvalidActions && !isPending;

  useConfirmShortcut({
    isOpen: true,
    canConfirm: canSubmit,
    onConfirm: handleSubmit,
  });

  return (
    <Dialog
      icon={editingRuleId ? 'edit' : 'add'}
      title={editingRuleId ? 'Edit Rule' : 'Add Rule'}
      isOpen
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
              <option value="blueprint">Device Blueprint</option>
              <option value="fleet">Fleet</option>
              <option value="device">Device</option>
              <option value="device_type">Legacy Device Type</option>
            </HTMLSelect>
          </FormGroup>
        </div>

        {targetType === 'blueprint' && (
          <FormGroup label="Device Blueprint">
            <HTMLSelect fill value={targetId} onChange={(e) => setTargetId(e.target.value)}>
              <option value="">Select blueprint...</option>
              {blueprints.map((blueprint) => (
                <option key={blueprint.id} value={blueprint.id}>
                  {blueprint.name}
                </option>
              ))}
            </HTMLSelect>
          </FormGroup>
        )}

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

        <RuleConditions
          conditions={conditions}
          triggerType={triggerType}
          telemetryFields={telemetryFields}
          zones={zones}
          setConditions={setConditions}
          updateCondition={updateCondition}
          addCondition={addCondition}
          removeCondition={removeCondition}
        />
        <RuleActions
          actions={actions}
          commandOptions={commandOptions}
          updateAction={updateAction}
          changeActionType={changeActionType}
          addAction={addAction}
          removeAction={removeAction}
        />

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
