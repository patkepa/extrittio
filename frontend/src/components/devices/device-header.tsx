import { H4, Tooltip } from '@blueprintjs/core';
import { showSuccessToast } from '../../utils/toaster';
import type { Device } from '../../types/api';

interface DeviceHeaderProps {
  device: Device;
}

export const DeviceHeader = ({ device }: DeviceHeaderProps) => (
  <div className="device-status-banner">
    <span className={`status-led status-led--${device.status}`} style={{ width: 10, height: 10 }} />
    <div>
      <H4 style={{ margin: 0 }}>{device.name}</H4>
      <p style={{ margin: 0 }} className="banner-subtitle">
        <Tooltip content="Click to copy" placement="top" compact minimal>
          <span
            className="mono-data copy-on-click"
            onClick={() => void navigator.clipboard.writeText(device.id).then(() => showSuccessToast('Device ID copied'))}
          >
            {device.id}
          </span>
        </Tooltip>
        <span className="banner-sep">|</span>
        {device.device_type_name}
        {device.fleet_name && (
          <>
            <span className="banner-sep">|</span>
            {device.fleet_name}
          </>
        )}
        <span className="banner-sep">|</span>
        <span style={{ textTransform: 'uppercase', fontWeight: 700, fontSize: 12, letterSpacing: '0.06em' }}>
          {device.status}
        </span>
      </p>
    </div>
  </div>
);
