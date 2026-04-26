import { useRef, useState } from 'react';
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
import {
  useFirmwareUpdates,
  useOtaDeployments,
  useTriggerOta,
} from '../../hooks/use-firmware-updates';
import { useFormNavigation } from '../../hooks/use-form-navigation';
import { showSuccessToast, showErrorToast } from '../../utils/toaster';
import { useDeviceShadow } from '../../hooks/use-shadow';
import type { Device } from '../../types/api';

interface OtaTabProps {
  device: Device;
}

export const OtaTab = ({ device }: OtaTabProps) => {
  const formRef = useRef<HTMLDivElement | null>(null);
  const {
    data: firmwareUpdates = [],
    isLoading,
    isError,
  } = useFirmwareUpdates({
    device_type_id: device.device_type_id,
  });
  const { data: shadow } = useDeviceShadow(device.id);
  const { data: deployments = [] } = useOtaDeployments(device.id);
  const triggerOtaMutation = useTriggerOta();
  const [selectedFwId, setSelectedFwId] = useState<number | null>(null);
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);
  useFormNavigation(formRef);

  const pendingOta = shadow?.desired?.ota as
    | { firmware_version?: string; firmware_url?: string }
    | undefined;
  const reportedOta = shadow?.reported?.ota as
    | { firmware_version?: string; status?: string }
    | undefined;

  const hasPendingOta = !!pendingOta?.firmware_version;
  const otaInDelta = shadow?.delta?.ota !== undefined;

  if (isLoading) return <Spinner />;

  if (isError) {
    return (
      <Callout intent="danger" icon="error">
        Failed to load firmware data. Try refreshing the page.
      </Callout>
    );
  }

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
      },
    );
  };

  const statusIntent = (status: string) => {
    switch (status) {
      case 'success':
        return 'success' as const;
      case 'failed':
        return 'danger' as const;
      case 'pending':
        return 'warning' as const;
      default:
        return 'primary' as const;
    }
  };

  return (
    <div className="ota-tab" ref={formRef}>
      {/* Current firmware status */}
      <Card elevation={Elevation.ONE} className="tab-card">
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
            <Tag intent={statusIntent(reportedOta.status)} minimal>
              {reportedOta.status}
            </Tag>
          )}
        </div>

        {hasPendingOta && (
          <div style={{ marginTop: 12, fontSize: 13 }}>
            <span className="tab-label-muted">Target version: </span>
            <span className="mono-data">{pendingOta!.firmware_version}</span>
          </div>
        )}
      </Card>

      <Divider className="tab-divider" />

      {/* Deploy new firmware */}
      <span className="section-label">Deploy Firmware Update</span>
      <p className="tab-help-text">
        Select a firmware release to push via device shadow. The device will receive the update URL
        in its shadow delta.
      </p>

      {firmwareUpdates.length === 0 ? (
        <Callout icon="info-sign" intent="primary">
          No firmware releases registered for device type "{device.device_type_name}". Register one
          in Settings &rarr; Firmware.
        </Callout>
      ) : (
        <>
          <HTMLSelect
            value={selectedFwId ?? ''}
            onChange={(e) => setSelectedFwId(e.target.value ? Number(e.target.value) : null)}
            fill
            className="tab-input-spacing"
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
            <Card elevation={Elevation.ONE} className="tab-card-sm">
              <div style={{ fontSize: 13 }}>
                <div>
                  <span className="tab-label-muted">Version: </span>
                  <span className="mono-data">v{selectedFw.version}</span>
                </div>
                {selectedFw.has_blob ? (
                  <div>
                    <span className="tab-label-muted">Source: </span>
                    <Tag
                      minimal
                      intent="success"
                      icon="document"
                      style={{ verticalAlign: 'middle' }}
                    >
                      {selectedFw.filename}
                    </Tag>
                  </div>
                ) : (
                  <div>
                    <span className="tab-label-muted">URL: </span>
                    <span className="mono-data" style={{ fontSize: 12 }}>
                      {selectedFw.url}
                    </span>
                  </div>
                )}
                {selectedFw.sha256 && (
                  <div>
                    <span className="tab-label-muted">SHA-256: </span>
                    <span className="mono-data" style={{ fontSize: 12 }}>
                      {selectedFw.sha256}
                    </span>
                  </div>
                )}
                {selectedFw.description && (
                  <div style={{ marginTop: 4 }}>
                    <span className="tab-label-muted">Notes: </span>
                    {selectedFw.description}
                  </div>
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
              Push firmware <strong>v{selectedFw?.version}</strong> to device{' '}
              <strong>{device.name}</strong>?
            </p>
            <p style={{ fontSize: 12, opacity: 0.7 }}>
              This will update the device shadow's desired state. The device will download and apply
              the firmware on its next sync.
            </p>
          </Alert>
        </>
      )}

      {/* Deployment History */}
      {deployments.length > 0 && (
        <>
          <Divider className="tab-divider-lg" />
          <span className="section-label">Deployment History</span>
          <Card elevation={Elevation.ONE} className="tab-card" style={{ marginTop: 8 }}>
            <HTMLTable compact className="tab-table">
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
                        <span className="tab-text-secondary">{dep.error_message}</span>
                      )}
                    </td>
                    <td className="tab-cell-muted">{dep.initiated_at}</td>
                    <td className="tab-cell-muted">{dep.completed_at ?? '—'}</td>
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
