import { Button } from '@blueprintjs/core';
import { useNavigate } from 'react-router-dom';
import type { Device } from '../../types/api';

interface DevicePopoverProps {
  device: Device;
  position: { x: number; y: number };
  onClose: () => void;
}

export const DevicePopover = ({ device, position, onClose }: DevicePopoverProps) => {
  const navigate = useNavigate();

  return (
    <>
      <div className="fleet-graph-popover-backdrop" onMouseDown={onClose} />
      <div
        className="fleet-graph-popover"
        style={{ left: position.x, top: position.y }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="popover-header">
          <strong>{device.name}</strong>
          <Button icon="cross" minimal small onClick={onClose} />
        </div>
        <div className="popover-body">
          <div className="popover-row">
            <span className="popover-label">Type</span>
            <span>{device.device_type_name}</span>
          </div>
          <div className="popover-row">
            <span className="popover-label">Fleet</span>
            <span>{device.fleet_name ?? '—'}</span>
          </div>
          <div className="popover-row">
            <span className="popover-label">Status</span>
            <span className="popover-status">
              <span className={`status-led status-led--${device.status}`} />
              {device.status}
            </span>
          </div>
          <div className="popover-row">
            <span className="popover-label">Firmware</span>
            <code className="mono-data">{device.firmware}</code>
          </div>
          <div className="popover-row">
            <span className="popover-label">Last Seen</span>
            <span className="mono-data">{device.last_seen}</span>
          </div>
          <div className="popover-row">
            <span className="popover-label">Uptime</span>
            <span className="mono-data">{device.uptime}</span>
          </div>
        </div>
        <div className="popover-footer">
          <Button
            intent="primary"
            small
            icon="eye-open"
            onClick={() => navigate(`/devices/${device.id}`)}
          >
            View Details
          </Button>
        </div>
      </div>
    </>
  );
};
