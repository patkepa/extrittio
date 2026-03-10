import { useState } from 'react';
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

interface CommandsTabProps {
  deviceId: string;
}

export const CommandsTab = ({ deviceId }: CommandsTabProps) => {
  const { data: commands = [], isLoading } = useCommandHistory(deviceId);
  const sendCommandMutation = useSendCommand();

  const [commandName, setCommandName] = useState('');
  const [params, setParams] = useState<Array<{ key: string; value: string }>>([]);
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  const addParam = () => setParams([...params, { key: '', value: '' }]);

  const updateParam = (index: number, field: 'key' | 'value', value: string) => {
    setParams(params.map((p, i) => (i === index ? { ...p, [field]: value } : p)));
  };

  const removeParam = (index: number) => {
    setParams(params.filter((_, i) => i !== index));
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
        },
        onError: () => {
          setIsConfirmOpen(false);
        },
      }
    );
  };

  const statusIntent = (status: string) => {
    switch (status) {
      case 'succeeded': return 'success' as const;
      case 'failed': return 'danger' as const;
      case 'timed_out': return 'warning' as const;
      case 'sent':
      case 'delivered': return 'primary' as const;
      default: return 'none' as const;
    }
  };

  if (isLoading) return <Spinner />;

  return (
    <div className="commands-tab">
      <Card elevation={Elevation.ONE} style={{ marginBottom: 16, padding: 16, backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}>
        <span className="section-label">Send Command</span>
        <p style={{ fontSize: 12, opacity: 0.6, margin: '4px 0 12px' }}>
          Send a direct command to the device via Zenoh. The device must be online and listening.
        </p>

        <InputGroup
          placeholder="e.g. restart, get_diagnostics, set_mode"
          value={commandName}
          onChange={(e) => setCommandName(e.target.value)}
          style={{ marginBottom: 12 }}
        />

        {params.map((p, i) => (
          <div key={i} style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
            <InputGroup
              placeholder="Key"
              value={p.key}
              onChange={(e) => updateParam(i, 'key', e.target.value)}
              style={{ flex: 1 }}
            />
            <InputGroup
              placeholder="Value"
              value={p.value}
              onChange={(e) => updateParam(i, 'value', e.target.value)}
              style={{ flex: 1 }}
            />
            <Button minimal icon="cross" onClick={() => removeParam(i)} />
          </div>
        ))}

        <div style={{ display: 'flex', gap: 8 }}>
          <Button minimal icon="plus" onClick={addParam}>
            Add Parameter
          </Button>
        </div>

        <div style={{ marginTop: 12 }}>
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
              With {params.filter(p => p.key.trim()).length} parameter(s)
            </p>
          )}
        </Alert>

        {sendCommandMutation.isError && (
          <Callout intent="danger" icon="error" style={{ marginTop: 12 }}>
            Failed to send command. The device may be offline.
          </Callout>
        )}

        {sendCommandMutation.isSuccess && (
          <Callout intent="success" icon="tick" style={{ marginTop: 12 }}>
            Command sent. Check the history below for status updates.
          </Callout>
        )}
      </Card>

      <Divider style={{ margin: '16px 0' }} />

      <span className="section-label">Command History</span>
      <p style={{ fontSize: 12, opacity: 0.6, margin: '4px 0 12px' }}>
        Auto-refreshes every 5 seconds.
      </p>

      {commands.length === 0 ? (
        <Callout icon="info-sign" intent="primary" style={{ marginTop: 8 }}>
          No commands have been sent to this device yet.
        </Callout>
      ) : (
        <Card elevation={Elevation.ONE} style={{ backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}>
          <HTMLTable compact style={{ width: '100%' }}>
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
                      <span style={{ fontSize: 11, opacity: 0.6, marginLeft: 6 }}>
                        {Object.entries(cmd.params).map(([k, v]) => `${k}=${v}`).join(', ')}
                      </span>
                    )}
                  </td>
                  <td>
                    <Tag minimal intent={statusIntent(cmd.status)}>
                      {cmd.status}
                    </Tag>
                  </td>
                  <td style={{ fontSize: 12, opacity: 0.8 }}>{cmd.created_at}</td>
                  <td style={{ fontSize: 12, opacity: 0.8 }}>{cmd.updated_at}</td>
                  <td style={{ fontSize: 12, opacity: 0.8 }}>
                    {cmd.response_payload ? (
                      <code style={{ fontSize: 11 }}>
                        {JSON.stringify(cmd.response_payload)}
                      </code>
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
