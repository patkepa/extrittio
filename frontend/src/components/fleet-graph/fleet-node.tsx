import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Icon } from '@blueprintjs/core';

interface FleetNodeData {
  label: string;
  deviceCount: number;
  [key: string]: unknown;
}

export const FleetNode = memo(({ data }: { data: FleetNodeData }) => {
  return (
    <div className="fleet-graph-fleet-node">
      <Handle type="source" position={Position.Top} style={{ visibility: 'hidden' }} />
      <Icon icon="layers" size={14} className="fleet-node-icon" />
      <span className="fleet-node-label">{data.label}</span>
      <span className="fleet-node-count mono-data">{data.deviceCount}</span>
      <Handle type="source" position={Position.Bottom} style={{ visibility: 'hidden' }} />
      <Handle type="source" position={Position.Left} style={{ visibility: 'hidden' }} />
      <Handle type="source" position={Position.Right} style={{ visibility: 'hidden' }} />
    </div>
  );
});

FleetNode.displayName = 'FleetNode';
