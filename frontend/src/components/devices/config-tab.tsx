import { useRef, useState } from 'react';
import { Button, Callout, Divider, InputGroup, Spinner } from '@blueprintjs/core';
import { useDeviceConfig, useUpdateDeviceConfig } from '../../hooks/use-config';
import { useFormNavigation } from '../../lib/interactions';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

interface ConfigTabProps {
  deviceId: string;
}

export const ConfigTab = ({ deviceId }: ConfigTabProps) => {
  const formRef = useRef<HTMLDivElement | null>(null);
  const { data: configData, isLoading, isError } = useDeviceConfig(deviceId);
  const updateMutation = useUpdateDeviceConfig();
  const [newKey, setNewKey] = useState('');
  const [newValue, setNewValue] = useState('');
  useFormNavigation(formRef);

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load configuration. Try refreshing the page.
      </Callout>
    );
  }

  const config = configData?.config ?? {};
  const entries = Object.entries(config).sort(([a], [b]) => a.localeCompare(b));
  const hasEntries = entries.length > 0;

  const handleAdd = () => {
    const key = newKey.trim();
    if (!key) return;

    // Try to parse as JSON value, fall back to string
    let parsedValue: unknown;
    try {
      parsedValue = JSON.parse(newValue);
    } catch {
      parsedValue = newValue;
    }

    updateMutation.mutate(
      { deviceId, config: { [key]: parsedValue } },
      {
        onSuccess: () => {
          setNewKey('');
          setNewValue('');
          void showSuccessToast('Config updated');
        },
        onError: () => {
          void showErrorToast('Failed to update config');
        },
      },
    );
  };

  const handleRemove = (key: string) => {
    updateMutation.mutate(
      { deviceId, config: { [key]: null } },
      {
        onSuccess: () => void showSuccessToast('Config entry removed'),
        onError: () => void showErrorToast('Failed to update config'),
      },
    );
  };

  return (
    <div className="config-tab" ref={formRef}>
      {!hasEntries && (
        <Callout icon="info-sign" intent="primary" style={{ marginBottom: 16 }}>
          No configuration entries yet. Add key-value pairs below.
        </Callout>
      )}

      {hasEntries && (
        <div className="config-table">
          <div className="config-row config-header">
            <span className="config-key section-label">Key</span>
            <span className="config-value section-label">Value</span>
            <span className="config-actions section-label" />
          </div>
          {entries.map(([key, value]) => (
            <div key={key} className="config-row">
              <span className="config-key mono-data">{key}</span>
              <span className="config-value mono-data">
                {typeof value === 'object' ? JSON.stringify(value) : String(value)}
              </span>
              <span className="config-actions">
                <Button
                  icon="cross"
                  minimal
                  small
                  intent="danger"
                  loading={updateMutation.isPending}
                  onClick={() => handleRemove(key)}
                />
              </span>
            </div>
          ))}
        </div>
      )}

      <Divider className="tab-divider" />

      <span className="section-label">Add Configuration Entry</span>
      <div className="config-add-form">
        <InputGroup
          placeholder="Key"
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          className="mono-data"
        />
        <InputGroup
          placeholder="Value"
          value={newValue}
          onChange={(e) => setNewValue(e.target.value)}
          className="mono-data"
        />
        <Button
          intent="primary"
          icon="plus"
          loading={updateMutation.isPending}
          disabled={!newKey.trim()}
          onClick={handleAdd}
        >
          Add
        </Button>
      </div>

      {updateMutation.isError && (
        <Callout intent="danger" icon="error" className="tab-callout">
          Failed to update configuration.
        </Callout>
      )}
    </div>
  );
};
