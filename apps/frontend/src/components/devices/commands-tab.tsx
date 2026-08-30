import { useRef, useState } from 'react';
import {
  Alert,
  Button,
  Callout,
  Card,
  Divider,
  Elevation,
  FormGroup,
  HTMLSelect,
  HTMLTable,
  InputGroup,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import { useCommandHistory, useSendCommand } from '../../hooks/use-commands';
import { useFormNavigation } from '@patkepa/kantzen-ui/interactions';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import { hasPermission } from '../../auth/permissions';
import { useAuthStore } from '../../stores/auth-store';
import { useDeviceContract } from '../../hooks/use-devices';

interface CommandsTabProps {
  deviceId: string;
}

interface CommandInputProperty {
  type?: 'number' | 'integer' | 'string' | 'boolean';
  title?: string;
  description?: string;
  default?: unknown;
}

interface ContractCommand {
  label: string;
  description?: string;
  danger: 'normal' | 'confirm' | 'critical';
  inputSchema: {
    properties?: Record<string, CommandInputProperty>;
    required?: string[];
  };
}

export const CommandsTab = ({ deviceId }: CommandsTabProps) => {
  const formRef = useRef<HTMLDivElement | null>(null);
  const permissions = useAuthStore((s) => s.user?.permissions);
  const canSendCommands = hasPermission(permissions, 'commands.send');
  const { data: commands = [], isLoading, isError } = useCommandHistory(deviceId);
  const contractQuery = useDeviceContract(deviceId);
  const sendCommandMutation = useSendCommand();

  const [commandName, setCommandName] = useState('');
  const [params, setParams] = useState<Record<string, string>>({});
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);
  useFormNavigation(formRef);
  const contractDocument = contractQuery.data?.document as unknown as {
    commands?: Record<string, ContractCommand>;
  };
  const commandDefinitions = contractDocument?.commands ?? {};
  const commandEntries = Object.entries(commandDefinitions);
  const selectedCommandName = commandName || commandEntries[0]?.[0] || '';
  const selectedCommand = commandDefinitions[selectedCommandName];
  const inputProperties = Object.entries(selectedCommand?.inputSchema.properties ?? {});
  const requiredInputs = new Set(selectedCommand?.inputSchema.required ?? []);
  const commandInputValid = inputProperties.every(
    ([key]) => !requiredInputs.has(key) || (params[key] ?? '').trim().length > 0,
  );

  const handleSend = () => {
    if (!selectedCommandName || !selectedCommand) return;
    const paramsMap = Object.fromEntries(
      inputProperties
        .filter(([key]) => (params[key] ?? '').length > 0)
        .map(([key, definition]) => {
          const raw = params[key] ?? '';
          if (definition.type === 'number' || definition.type === 'integer') {
            return [key, Number(raw)];
          }
          if (definition.type === 'boolean') return [key, raw === 'true'];
          return [key, raw];
        }),
    );

    sendCommandMutation.mutate(
      {
        deviceId,
        body: {
          command: selectedCommandName,
          params: paramsMap,
        },
      },
      {
        onSuccess: () => {
          setCommandName('');
          setParams({});
          setIsConfirmOpen(false);
          void showSuccessToast('Command sent');
        },
        onError: () => {
          setIsConfirmOpen(false);
          void showErrorToast('Failed to send command');
        },
      },
    );
  };

  const statusIntent = (status: string) => {
    switch (status) {
      case 'succeeded':
        return 'success' as const;
      case 'failed':
        return 'danger' as const;
      case 'timed_out':
        return 'warning' as const;
      case 'sent':
      case 'delivered':
        return 'primary' as const;
      default:
        return 'none' as const;
    }
  };

  if (isLoading || contractQuery.isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load command history. Try refreshing the page.
      </Callout>
    );
  }

  return (
    <div className="commands-tab" ref={formRef}>
      {canSendCommands && (
        <Card elevation={Elevation.ONE} className="tab-card">
          <span className="section-label">Send Command</span>
          <p className="tab-help-text">
            Commands and their inputs come from the device's assigned contract.
          </p>
          {contractQuery.isError || commandEntries.length === 0 ? (
            <Callout intent="warning" icon="warning-sign" className="tab-callout">
              This device contract does not declare any commands.
            </Callout>
          ) : (
            <>
              <FormGroup label="Command" labelInfo="(required)">
                <HTMLSelect
                  fill
                  value={selectedCommandName}
                  onChange={(event) => {
                    setCommandName(event.target.value);
                    setParams({});
                  }}
                >
                  {commandEntries.map(([key, command]) => (
                    <option key={key} value={key}>
                      {command.label}
                    </option>
                  ))}
                </HTMLSelect>
              </FormGroup>
              {selectedCommand?.description && (
                <p className="tab-help-text">{selectedCommand.description}</p>
              )}
              {inputProperties.map(([key, definition]) => (
                <FormGroup
                  key={key}
                  label={definition.title ?? key}
                  labelInfo={requiredInputs.has(key) ? '(required)' : '(optional)'}
                  helperText={definition.description}
                >
                  {definition.type === 'boolean' ? (
                    <HTMLSelect
                      fill
                      value={params[key] ?? ''}
                      onChange={(event) => setParams({ ...params, [key]: event.target.value })}
                    >
                      {!requiredInputs.has(key) && <option value="">Use default</option>}
                      <option value="true">True</option>
                      <option value="false">False</option>
                    </HTMLSelect>
                  ) : (
                    <InputGroup
                      type={
                        definition.type === 'number' || definition.type === 'integer'
                          ? 'number'
                          : 'text'
                      }
                      placeholder={
                        definition.default == null ? undefined : String(definition.default)
                      }
                      value={params[key] ?? ''}
                      onChange={(event) => setParams({ ...params, [key]: event.target.value })}
                    />
                  )}
                </FormGroup>
              ))}
            </>
          )}

          <div className="tab-callout">
            <Button
              intent="primary"
              icon="send-message"
              loading={sendCommandMutation.isPending}
              disabled={!selectedCommandName || !commandInputValid}
              onClick={() => setIsConfirmOpen(true)}
            >
              Send Command
            </Button>
          </div>

          <Alert
            isOpen={isConfirmOpen}
            onClose={() => setIsConfirmOpen(false)}
            onConfirm={handleSend}
            cancelButtonText="Cancel"
            confirmButtonText="Send"
            intent="primary"
            icon="send-message"
            loading={sendCommandMutation.isPending}
          >
            <p>
              Send command <strong>{selectedCommand?.label ?? selectedCommandName}</strong> to this
              device?
            </p>
            {Object.keys(params).length > 0 && (
              <p style={{ fontSize: 12, opacity: 0.7 }}>
                With {Object.keys(params).length} parameter(s)
              </p>
            )}
          </Alert>
        </Card>
      )}

      {canSendCommands && <Divider className="tab-divider" />}

      <span className="section-label">Command History</span>
      <p className="tab-help-text">Auto-refreshes every 5 seconds.</p>

      {commands.length === 0 ? (
        <Callout icon="info-sign" intent="primary" className="tab-callout">
          No commands have been sent to this device yet.
        </Callout>
      ) : (
        <Card elevation={Elevation.ONE} className="tab-card">
          <HTMLTable compact className="tab-table">
            <thead>
              <tr>
                <th>Command</th>
                <th>Status</th>
                <th>Sent</th>
                <th>Updated</th>
                <th>Response</th>
              </tr>
            </thead>
            <tbody>
              {commands.map((cmd) => (
                <tr key={cmd.id}>
                  <td>
                    <Tag minimal intent="primary" className="mono-data">
                      {cmd.command}
                    </Tag>
                    {cmd.params && Object.keys(cmd.params).length > 0 && (
                      <span className="tab-text-secondary">
                        {Object.entries(cmd.params)
                          .map(([k, v]) => `${k}=${v}`)
                          .join(', ')}
                      </span>
                    )}
                  </td>
                  <td>
                    <Tag minimal intent={statusIntent(cmd.status)}>
                      {cmd.status}
                    </Tag>
                  </td>
                  <td className="tab-cell-muted">{cmd.created_at}</td>
                  <td className="tab-cell-muted">{cmd.updated_at}</td>
                  <td className="tab-cell-muted">
                    {cmd.response_payload ? (
                      <code style={{ fontSize: 11 }}>{JSON.stringify(cmd.response_payload)}</code>
                    ) : (
                      '—'
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </HTMLTable>
        </Card>
      )}
    </div>
  );
};
