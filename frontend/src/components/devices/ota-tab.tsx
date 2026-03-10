import { useState } from 'react';
import {
  Alert,
  Button,
  Callout,
  Card,
  Divider,
  Elevation,
  HTMLSelect,
  HTMLTable,
  Spinner,
  Tag,
} from '@blueprintjs/core';
import { useFirmwareUpdates, useOtaDeployments, useTriggerOta } from '../../hooks/use-firmware-updates';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import { useDeviceShadow } from '../../hooks/use-shadow';
import type { Device } from '../../types/api';

interface OtaTabProps {
  device: Device;
}

export const OtaTab = ({ device }: OtaTabProps) => {
  const { data: firmwareUpdates = [], isLoading } = useFirmwareUpdates({
    device_type_id: device.device_type_id,
  });
  const { data: shadow } = useDeviceShadow(device.id);
  const { data: deployments = [] } = useOtaDeployments(device.id);
  const triggerOtaMutation = useTriggerOta();
  const [selectedFwId, setSelectedFwId] = useState<number | null>(null);
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);

  const pendingOta = shadow?.desired?.ota as
    | { firmware_version?: string; firmware_url?: string }
    | undefined;
  const reportedOta = shadow?.reported?.ota as
    | { firmware_version?: string; status?: string }
    | undefined;

  const hasPendingOta = !!pendingOta?.firmware_version;
  const otaInDelta = shadow?.delta?.ota !== undefined;

  if (isLoading) return <Spinner />;

  const selectedFw = firmwareUpdates.find((f) => f.id === selectedFwId);

  const handleTrigger = () => {
    if (!selectedFwId) return;
    triggerOtaMutation.mutate(
      { deviceId: device.id, body: { firmware_update_id: selectedFwId } },
      {
        onSuccess: () => {
          setSelectedFwId(null);
          setIsConfirmOpen(false);
          void showSuccessToast('OTA update pushed');
        },
        onError: () => {
          setIsConfirmOpen(false);
          void showErrorToast('Failed to trigger OTA update');
        },
      }
    );
  };

  const statusIntent = (status: string) => {
    switch (status) {
      case 'success': return 'success' as const;
      case 'failed': return 'danger' as const;
      case 'pending': return 'warning' as const;
      default: return 'primary' as const;
    }
  };

  return (
    <div className="ota-tab">
      {/* Current firmware status */}
      <Card elevation={Elevation.ONE} style={{ marginBottom: 16, padding: 16, backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}>
        <span className="section-label">Current Firmware</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginTop: 8 }}>
          <Tag minimal large className="mono-data">
            {device.firmware || 'unknown'}
          </Tag>
          {hasPendingOta && (
            <Tag intent={otaInDelta ? 'warning' : 'success'} minimal>
              {otaInDelta ? 'OTA Pending' : 'OTA Delivered'}
            </Tag>
          )}
          {reportedOta?.status && (
            <Tag
              intent={statusIntent(reportedOta.status)}
              minimal
            >
              {reportedOta.status}
            </Tag>
          )}
        </div>

        {hasPendingOta && (
          <div style={{ marginTop: 12, fontSize: 13 }}>
            <span style={{ opacity: 0.6 }}>Target version: </span>
            <span className="mono-data">{pendingOta!.firmware_version}</span>
          </div>
        )}
      </Card>

      <Divider style={{ margin: '16px 0' }} />

      {/* Deploy new firmware */}
      <span className="section-label">Deploy Firmware Update</span>
      <p style={{ fontSize: 12, opacity: 0.6, margin: '4px 0 12px' }}>
        Select a firmware release to push via device shadow. The device will receive the update URL in its shadow delta.
      </p>

      {firmwareUpdates.length === 0 ? (
        <Callout icon="info-sign" intent="primary">
          No firmware releases registered for device type "{device.device_type_name}".
          Register one in Settings &rarr; Firmware.
        </Callout>
      ) : (
        <>
          <HTMLSelect
            value={selectedFwId ?? ''}
            onChange={(e) =>
              setSelectedFwId(e.target.value ? Number(e.target.value) : null)
            }
            fill
            style={{ marginBottom: 12 }}
          >
            <option value="">Select firmware version...</option>
            {firmwareUpdates.map((fw) => (
              <option key={fw.id} value={fw.id}>
                v{fw.version}
                {fw.description ? ` — ${fw.description}` : ''}
              </option>
            ))}
          </HTMLSelect>

          {selectedFw && (
            <Card elevation={Elevation.ONE} style={{ marginBottom: 12, padding: 12, backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}>
              <div style={{ fontSize: 13 }}>
                <div><span style={{ opacity: 0.6 }}>Version: </span><span className="mono-data">v{selectedFw.version}</span></div>
                {selectedFw.has_blob ? (
                  <div><span style={{ opacity: 0.6 }}>Source: </span><Tag minimal intent="success" icon="document" style={{ verticalAlign: 'middle' }}>{selectedFw.filename}</Tag></div>
                ) : (
                  <div><span style={{ opacity: 0.6 }}>URL: </span><span className="mono-data" style={{ fontSize: 12 }}>{selectedFw.url}</span></div>
                )}
                {selectedFw.sha256 && (
                  <div><span style={{ opacity: 0.6 }}>SHA-256: </span><span className="mono-data" style={{ fontSize: 12 }}>{selectedFw.sha256}</span></div>
                )}
                {selectedFw.description && (
                  <div style={{ marginTop: 4 }}><span style={{ opacity: 0.6 }}>Notes: </span>{selectedFw.description}</div>
                )}
              </div>
            </Card>
          )}

          <Button
            intent="warning"
            icon="cloud-upload"
            loading={triggerOtaMutation.isPending}
            disabled={!selectedFwId}
            onClick={() => setIsConfirmOpen(true)}
          >
            Push OTA Update
          </Button>

          <Alert
            isOpen={isConfirmOpen}
            onClose={() => setIsConfirmOpen(false)}
            onConfirm={handleTrigger}
            cancelButtonText="Cancel"
            confirmButtonText="Push Update"
            intent="warning"
            icon="warning-sign"
            loading={triggerOtaMutation.isPending}
          >
            <p>
              Push firmware <strong>v{selectedFw?.version}</strong> to device <strong>{device.name}</strong>?
            </p>
            <p style={{ fontSize: 12, opacity: 0.7 }}>
              This will update the device shadow's desired state. The device will download and apply the firmware on its next sync.
            </p>
          </Alert>

        </>
      )}

      {/* Deployment History */}
      {deployments.length > 0 && (
        <>
          <Divider style={{ margin: '24px 0 16px' }} />
          <span className="section-label">Deployment History</span>
          <Card elevation={Elevation.ONE} style={{ marginTop: 8, backgroundColor: 'var(--card-bg)', border: '1px solid var(--border-color)' }}>
            <HTMLTable compact style={{ width: '100%' }}>
              <thead>
                <tr>
                  <th>Version</th>
                  <th>Status</th>
                  <th>Initiated</th>
                  <th>Completed</th>
                </tr>
              </thead>
              <tbody>
                {deployments.map((dep) => (
                  <tr key={dep.id}>
                    <td>
                      <Tag minimal intent="primary" className="mono-data">
                        v{dep.firmware_version}
                      </Tag>
                    </td>
                    <td>
                      <Tag minimal intent={statusIntent(dep.status)}>
                        {dep.status}
                      </Tag>
                      {dep.error_message && (
                        <span style={{ fontSize: 11, opacity: 0.7, marginLeft: 6 }}>
                          {dep.error_message}
                        </span>
                      )}
                    </td>
                    <td style={{ fontSize: 12, opacity: 0.8 }}>{dep.initiated_at}</td>
                    <td style={{ fontSize: 12, opacity: 0.8 }}>{dep.completed_at ?? '—'}</td>
                  </tr>
                ))}
              </tbody>
            </HTMLTable>
          </Card>
        </>
      )}
    </div>
  );
};
