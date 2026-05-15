import { useRef, useState } from 'react';
import {
  Alert,
  Button,
  Callout,
  Card,
  Divider,
  Elevation,
  HTMLTable,
  InputGroup,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import { useCommandHistory, useSendCommand } from '../../hooks/use-commands';
import { useFormNavigation } from '@extrittio/interactions';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import { hasPermission } from '../../auth/permissions';
import { useAuthStore } from '../../stores/auth-store';

interface CommandsTabProps {
  deviceId: string;
}

export const CommandsTab = ({ deviceId }: CommandsTabProps) => {
  const formRef = useRef<HTMLDivElement | null>(null);
  const permissions = useAuthStore((s) => s.user?.permissions);
  const canSendCommands = hasPermission(permissions, 'commands.send');
  const { data: commands = [], isLoading, isError } = useCommandHistory(deviceId);
  const sendCommandMutation = useSendCommand();

  const [commandName, setCommandName] = useState('');
  const [params, setParams] = useState<Array<{ id: number; key: string; value: string }>>([]);
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);
  const nextParamId = useRef(0);
  useFormNavigation(formRef);

  const addParam = () => setParams([...params, { id: nextParamId.current++, key: '', value: '' }]);

  const updateParam = (id: number, field: 'key' | 'value', value: string) => {
    setParams(params.map((p) => (p.id === id ? { ...p, [field]: value } : p)));
  };

  const removeParam = (id: number) => {
    setParams(params.filter((p) => p.id !== id));
  };

  const handleSend = () => {
    if (!commandName.trim()) return;

    const paramsMap: Record<string, string> = {};
    for (const p of params) {
      if (p.key.trim()) {
        paramsMap[p.key.trim()] = p.value;
      }
    }

    sendCommandMutation.mutate(
      {
        deviceId,
        body: {
          command: commandName.trim(),
          params: Object.keys(paramsMap).length > 0 ? paramsMap : undefined,
        },
      },
      {
        onSuccess: () => {
          setCommandName('');
          setParams([]);
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

  if (isLoading) return <Spinner />;

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
            Send a direct command to the device via Zenoh. The device must be online and listening.
          </p>

          <InputGroup
            placeholder="e.g. restart, get_diagnostics, set_mode"
            value={commandName}
            onChange={(e) => setCommandName(e.target.value)}
            className="tab-input-spacing"
          />

          {params.map((p) => (
            <div key={p.id} className="tab-param-row">
              <InputGroup
                placeholder="Key"
                value={p.key}
                onChange={(e) => updateParam(p.id, 'key', e.target.value)}
                style={{ flex: 1 }}
              />
              <InputGroup
                placeholder="Value"
                value={p.value}
                onChange={(e) => updateParam(p.id, 'value', e.target.value)}
                style={{ flex: 1 }}
              />
              <Button minimal icon="cross" onClick={() => removeParam(p.id)} />
            </div>
          ))}

          <div className="tab-actions">
            <Button minimal icon="plus" onClick={addParam}>
              Add Parameter
            </Button>
          </div>

          <div className="tab-callout">
            <Button
              intent="primary"
              icon="send-message"
              loading={sendCommandMutation.isPending}
              disabled={!commandName.trim()}
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
              Send command <strong>{commandName}</strong> to this device?
            </p>
            {params.length > 0 && (
              <p style={{ fontSize: 12, opacity: 0.7 }}>
                With {params.filter((p) => p.key.trim()).length} parameter(s)
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
