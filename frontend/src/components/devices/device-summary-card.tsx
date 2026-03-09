import { Tag } from '@blueprintjs/core';
import type { Device } from '../../types/api';

interface DeviceSummaryCardProps {
  device: Device;
}

export const DeviceSummaryCard = ({ device }: DeviceSummaryCardProps) => (
  <div className="device-summary-card">
    <div className="summary-header">
      <span className={`status-led status-led--${device.status}`} />
      <strong>{device.name}</strong>
    </div>
    <div className="summary-details">
      <Tag minimal>{device.device_type_name}</Tag>
      {device.fleet_name && (
        <Tag minimal intent="primary">{device.fleet_name}</Tag>
      )}
    </div>
    <div className="summary-meta">
      <span className="mono-data">Last seen: {device.last_seen}</span>
    </div>
  </div>
);
