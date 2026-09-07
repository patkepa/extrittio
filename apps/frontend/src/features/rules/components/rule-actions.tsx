import { Button, FormGroup, HTMLSelect, InputGroup, Icon } from '@blueprintjs/core';
import type { ActionRow } from '../model/rule-form';

const ACTION_TYPES = ['alert', 'webhook', 'command'];
const SEVERITY_OPTIONS = ['info', 'warning', 'critical'];
interface Props {
  actions: ActionRow[];
  commandOptions: { value: string; label: string }[];
  updateAction: (index: number, updates: Partial<ActionRow>) => void;
  changeActionType: (index: number, type: string) => void;
  addAction: () => void;
  removeAction: (index: number) => void;
}
export function RuleActions({
  actions,
  commandOptions,
  updateAction,
  changeActionType,
  addAction,
  removeAction,
}: Props) {
  return (
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
              {commandOptions.length > 0 ? (
                <HTMLSelect
                  fill
                  value={(action.config.command as string) ?? ''}
                  onChange={(e) =>
                    updateAction(i, { config: { ...action.config, command: e.target.value } })
                  }
                >
                  <option value="">Select command...</option>
                  {typeof action.config.command === 'string' &&
                    action.config.command.length > 0 &&
                    !commandOptions.some((command) => command.value === action.config.command) && (
                      <option value={String(action.config.command)}>
                        {String(action.config.command)} (existing)
                      </option>
                    )}
                  {commandOptions.map((command) => (
                    <option key={command.value} value={command.value}>
                      {command.label}
                    </option>
                  ))}
                </HTMLSelect>
              ) : (
                <InputGroup
                  placeholder="Contract command key..."
                  value={(action.config.command as string) ?? ''}
                  onChange={(e) =>
                    updateAction(i, { config: { ...action.config, command: e.target.value } })
                  }
                  leftIcon={<Icon icon="console" />}
                />
              )}
            </FormGroup>
          )}
        </div>
      ))}
    </div>
  );
}
