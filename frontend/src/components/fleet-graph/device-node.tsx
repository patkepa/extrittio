import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Icon } from '@blueprintjs/core';
import type { Device } from '../../types/api';

interface DeviceNodeData {
  device: Device;
  icon: string;
  [key: string]: unknown;
}

interface DeviceNodeProps {
  data: DeviceNodeData;
}

export const DeviceNode = memo(({ data }: DeviceNodeProps) => {
  const { device, icon } = data;

  return (
    <div className="fleet-graph-device-node">
      <Handle type="target" position={Position.Top} style={{ visibility: 'hidden' }} />
      <Handle type="target" position={Position.Bottom} style={{ visibility: 'hidden' }} />
      <Handle type="target" position={Position.Left} style={{ visibility: 'hidden' }} />
      <Handle type="target" position={Position.Right} style={{ visibility: 'hidden' }} />
      <Icon icon={icon as any} size={14} className="device-node-icon" />
      <span className="device-node-name">{device.name}</span>
      <span className={`status-led status-led--${device.status}`} />
    </div>
  );
});

DeviceNode.displayName = 'DeviceNode';
