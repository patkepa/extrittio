import { Button, HTMLSelect, InputGroup } from '@blueprintjs/core';
import type { ConditionRow } from '../model/rule-form';

const STATUS_FIELDS = [{ value: 'status', label: 'Status' }];

const GEOFENCE_FIELDS = [
  { label: 'Zone State', value: 'zone_state' },
  { label: 'Minimum Dwell (seconds)', value: 'dwell_seconds' },
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
const ZONE_STATE_OPERATORS = [{ value: 'eq', label: '=' }];
const STATUS_VALUES = ['online', 'offline', 'warning'];

interface Props {
  conditions: ConditionRow[];
  triggerType: string;
  telemetryFields: { value: string; label: string }[];
  zones: { id: string; name: string }[];
  updateCondition: (index: number, field: keyof ConditionRow, value: string) => void;
  addCondition: () => void;
  removeCondition: (index: number) => void;
}
export function RuleConditions({
  conditions,
  triggerType,
  telemetryFields,
  zones,
  updateCondition,
  addCondition,
  removeCondition,
}: Props) {
  return (
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
        <Button
          icon="add"
          minimal
          small
          onClick={addCondition}
          disabled={
            triggerType === 'geofence' &&
            (conditions.length >= 2 || conditions[0]?.value !== 'inside')
          }
        >
          Add
        </Button>
      </div>
      {conditions.map((cond, i) => {
        if (triggerType === 'geofence') {
          const isZoneState = cond.field === 'zone_state';
          const operators = isZoneState ? ZONE_STATE_OPERATORS : [{ value: 'gte', label: '>=' }];

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
                <HTMLSelect value={cond.field} disabled style={{ flex: 1 }}>
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
                  disabled={conditions.length <= 1 || cond.field === 'zone_state'}
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
        const fields = triggerType === 'device_status' ? STATUS_FIELDS : telemetryFields;
        const operators = triggerType === 'device_status' ? STATUS_OPERATORS : NUMERIC_OPERATORS;
        const isStatusField = cond.field === 'status';

        return (
          <div key={i} style={{ display: 'flex', gap: 8, marginBottom: 8, alignItems: 'center' }}>
            {triggerType === 'device_status' || fields.length > 0 ? (
              <HTMLSelect
                value={cond.field}
                onChange={(e) => updateCondition(i, 'field', e.target.value)}
                style={{ flex: 1 }}
              >
                {triggerType !== 'device_status' && <option value="">Select metric...</option>}
                {cond.field && !fields.some((field) => field.value === cond.field) && (
                  <option value={cond.field}>{cond.field} (existing)</option>
                )}
                {fields.map((f) => (
                  <option key={f.value} value={f.value}>
                    {f.label}
                  </option>
                ))}
              </HTMLSelect>
            ) : (
              <HTMLSelect value="" disabled style={{ flex: 1 }}>
                <option value="">Select a blueprint with numeric fields...</option>
              </HTMLSelect>
            )}
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
  );
}
