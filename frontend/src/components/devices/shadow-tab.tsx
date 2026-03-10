import { useState } from 'react';
import { Button, Callout, Divider, InputGroup, Spinner, Tag } from '@blueprintjs/core';
import { useDeviceShadow, useUpdateDesiredState, useDeleteDeviceShadow } from '../../hooks/use-shadow';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';

interface ShadowTabProps {
  deviceId: string;
}

export const ShadowTab = ({ deviceId }: ShadowTabProps) => {
  const { data: shadow, isLoading } = useDeviceShadow(deviceId);
  const updateDesiredMutation = useUpdateDesiredState();
  const deleteShadowMutation = useDeleteDeviceShadow();
  const [desiredInput, setDesiredInput] = useState('');

  if (isLoading) return <Spinner />;

  if (!shadow) {
    return <Callout icon="info-sign">No shadow data available.</Callout>;
  }

  const hasDelta = Object.keys(shadow.delta).length > 0;

  return (
    <div className="shadow-tab">
      <div className="shadow-status-row">
        <Tag intent={hasDelta ? 'warning' : 'success'} minimal large>
          {hasDelta ? 'Pending' : 'In Sync'}
        </Tag>
        <span className="mono-data" style={{ fontSize: 12, opacity: 0.6 }}>
          v{shadow.version}
        </span>
      </div>

      {hasDelta && (
        <Callout intent="warning" icon="info-sign" style={{ marginBottom: 16 }}>
          Delta: {Object.keys(shadow.delta).join(', ')}
        </Callout>
      )}

      <div className="shadow-panes-horizontal">
        <div className="shadow-pane">
          <span className="section-label">Reported State</span>
          <pre className="shadow-json mono-data">
            {JSON.stringify(shadow.reported, null, 2)}
          </pre>
        </div>
        <div className="shadow-pane">
          <span className="section-label">Desired State</span>
          <pre className="shadow-json mono-data">
            {JSON.stringify(shadow.desired, null, 2)}
          </pre>
        </div>
      </div>

      <Divider style={{ margin: '16px 0' }} />

      <span className="section-label">Update Desired State</span>
      <p style={{ fontSize: 12, opacity: 0.6, margin: '4px 0 8px' }}>
        Enter JSON to merge into desired state (e.g. {`{"interval": 30}`})
      </p>
      <InputGroup
        placeholder='{"key": "value"}'
        value={desiredInput}
        onChange={(e) => setDesiredInput(e.target.value)}
        className="mono-data"
      />
      <div className="shadow-actions" style={{ marginTop: 12, display: 'flex', gap: 8 }}>
        <Button
          intent="primary"
          icon="cloud-upload"
          loading={updateDesiredMutation.isPending}
          disabled={!desiredInput.trim()}
          onClick={() => {
            try {
              const parsed = JSON.parse(desiredInput);
              updateDesiredMutation.mutate(
                { deviceId, state: parsed },
                {
                  onSuccess: () => {
                    setDesiredInput('');
                    void showSuccessToast('Desired state updated');
                  },
                  onError: () => void showErrorToast('Failed to update desired state'),
                }
              );
            } catch {
              // Invalid JSON - ignore
            }
          }}
        >
          Send to Device
        </Button>
        <Button
          intent="danger"
          icon="trash"
          minimal
          loading={deleteShadowMutation.isPending}
          onClick={() =>
            deleteShadowMutation.mutate(deviceId, {
              onSuccess: () => void showSuccessToast('Shadow cleared'),
              onError: () => void showErrorToast('Failed to clear shadow'),
            })
          }
        >
          Clear Shadow
        </Button>
      </div>
    </div>
  );
};
