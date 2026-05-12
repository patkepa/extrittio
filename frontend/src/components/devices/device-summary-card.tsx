import { Tag } from '@blueprintjs/core';
import { StatusLed } from '@extrittio/ui';
import type { Device } from '../../types/api';
import { DeviceTypeTag } from './device-type-tag';

interface DeviceSummaryCardProps {
  device: Device;
}

export const DeviceSummaryCard = ({ device }: DeviceSummaryCardProps) => (
  <div className="device-summary-card">
    <div className="summary-header">
      <StatusLed status={device.status} />
      <strong>{device.name}</strong>
    </div>
    <div className="summary-details">
      <DeviceTypeTag
        name={device.device_type_name}
        icon={device.device_type_icon}
        colorHex={device.device_type_color_hex}
      />
      {device.fleet_name && (
        <Tag minimal intent="primary">
          {device.fleet_name}
        </Tag>
      )}
    </div>
    <div className="summary-meta">
      <span className="mono-data">Last seen: {device.last_seen}</span>
    </div>
  </div>
);
