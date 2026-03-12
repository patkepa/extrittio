import type { Node, Edge } from '@xyflow/react';
import type { Device, Fleet } from '../../types/api';

// --- Device type → Blueprint icon mapping ---
const DEVICE_TYPE_ICONS: Record<string, string> = {
  'mac-device': 'desktop',
  'linux': 'desktop',
  'esp32': 'pulse',
};

export function getDeviceTypeIcon(deviceTypeName: string): string {
  const key = deviceTypeName.toLowerCase();
  return DEVICE_TYPE_ICONS[key] ?? 'widget';
}

// --- Layout constants ---
const FLEET_RADIUS = 350;          // Distance between fleet hub centers
const DEVICE_RADIUS_BASE = 150;    // Base radius for device ring around fleet hub
const DEVICE_RADIUS_PER_ITEM = 20; // Extra radius per device (scales with count)
const FLEET_NODE_WIDTH = 160;
const FLEET_NODE_HEIGHT = 50;
const DEVICE_NODE_WIDTH = 200;
const DEVICE_NODE_HEIGHT = 44;

export interface FleetGraphData {
  nodes: Node[];
  edges: Edge[];
}

export function buildFleetGraph(
  devices: Device[],
  fleets: Fleet[],
): FleetGraphData {
  const nodes: Node[] = [];
  const edges: Edge[] = [];

  // Group devices by fleet_id (null → "unassigned")
  const devicesByFleet = new Map<number | null, Device[]>();
  for (const device of devices) {
    const key = device.fleet_id ?? null;
    if (!devicesByFleet.has(key)) {
      devicesByFleet.set(key, []);
    }
    devicesByFleet.get(key)!.push(device);
  }

  // Build list of fleet groups (real fleets + optional "Unassigned")
  interface FleetGroup {
    id: string;
    label: string;
    devices: Device[];
  }

  const groups: FleetGroup[] = [];

  for (const fleet of fleets) {
    groups.push({
      id: `fleet-${fleet.id}`,
      label: fleet.name,
      devices: devicesByFleet.get(fleet.id) ?? [],
    });
  }

  const unassigned = devicesByFleet.get(null);
  if (unassigned && unassigned.length > 0) {
    groups.push({
      id: 'fleet-unassigned',
      label: 'Unassigned',
      devices: unassigned,
    });
  }

  // Position fleet hubs in a circle (or single center if only one)
  const fleetCount = groups.length;

  groups.forEach((group, fi) => {
    // Fleet hub position
    let hubX: number, hubY: number;
    if (fleetCount === 1) {
      hubX = 0;
      hubY = 0;
    } else {
      const angle = (2 * Math.PI * fi) / fleetCount - Math.PI / 2;
      hubX = Math.cos(angle) * FLEET_RADIUS * Math.max(1, fleetCount / 3);
      hubY = Math.sin(angle) * FLEET_RADIUS * Math.max(1, fleetCount / 3);
    }

    // Fleet hub node
    nodes.push({
      id: group.id,
      type: 'fleetNode',
      position: {
        x: hubX - FLEET_NODE_WIDTH / 2,
        y: hubY - FLEET_NODE_HEIGHT / 2,
      },
      data: { label: group.label, deviceCount: group.devices.length },
    });

    // Device nodes arranged radially around their fleet hub
    const deviceCount = group.devices.length;
    const deviceRadius = DEVICE_RADIUS_BASE + deviceCount * DEVICE_RADIUS_PER_ITEM;

    group.devices.forEach((device, di) => {
      const angle = (2 * Math.PI * di) / deviceCount - Math.PI / 2;
      const dx = hubX + Math.cos(angle) * deviceRadius - DEVICE_NODE_WIDTH / 2;
      const dy = hubY + Math.sin(angle) * deviceRadius - DEVICE_NODE_HEIGHT / 2;

      const nodeId = `device-${device.id}`;

      nodes.push({
        id: nodeId,
        type: 'deviceNode',
        position: { x: dx, y: dy },
        data: {
          device,
          icon: getDeviceTypeIcon(device.device_type_name),
        },
      });

      edges.push({
        id: `edge-${group.id}-${device.id}`,
        source: group.id,
        target: nodeId,
        style: { stroke: 'hsl(0, 0%, 15%)', strokeWidth: 1 },
      });
    });
  });

  return { nodes, edges };
}
