import { Button, Callout, Divider } from '@blueprintjs/core';
import { useSearchParams } from 'react-router-dom';
import { useRestartDevice } from '../../hooks/use-devices';
import type { Device } from '../../types/api';

interface OverviewTabProps {
  device: Device;
}

export const OverviewTab = ({ device }: OverviewTabProps) => {
  const restartDeviceMutation = useRestartDevice();
  const [, setSearchParams] = useSearchParams();

  return (
    <>
      {device.status === 'offline' && (
        <Callout intent="danger" icon="error" style={{ marginBottom: 16 }}>
          Device offline — last seen {device.last_seen}
        </Callout>
      )}

      <div className="detail-grid">
        <div className="detail-item">
          <span className="section-label">Device ID</span>
          <span className="detail-value mono-data">{device.id}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Type</span>
          <span className="detail-value">{device.device_type_name}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Fleet</span>
          <span className="detail-value">{device.fleet_name ?? '—'}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Location</span>
          <span className="detail-value">{device.location}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Firmware</span>
          <span className="detail-value mono-data">{device.firmware}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Last Seen</span>
          <span className="detail-value mono-data">{device.last_seen}</span>
        </div>
        <div className="detail-item">
          <span className="section-label">Uptime</span>
          <span className="detail-value mono-data">{device.uptime}</span>
        </div>
      </div>

      <Divider style={{ margin: '16px 0' }} />

      <span className="section-label">Quick Actions</span>
      <div className="quick-actions">
        <Button
          icon="refresh"
          fill
          loading={restartDeviceMutation.isPending}
          onClick={() => restartDeviceMutation.mutate(device.id)}
        >
          Restart
        </Button>
        <Button icon="cloud-upload" fill onClick={() => setSearchParams({ tab: 'ota' })}>Update FW</Button>
        <Button icon="chart" fill>Telemetry</Button>
        <Button icon="cog" fill>Configure</Button>
      </div>
    </>
  );
};
