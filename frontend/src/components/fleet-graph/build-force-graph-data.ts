import type { Device, Fleet } from '../../types/api';

// --- Status → color mapping ---
const STATUS_COLORS: Record<string, string> = {
  online: '#0F9960',
  offline: '#E76A6E',
  warning: '#D9822B',
};

const FLEET_COLOR = '#2D72D2';
const DEFAULT_COLOR = '#555555';

// --- Device type → abbreviation ---
const TYPE_ABBREVS: Record<string, string> = {
  'mac-device': 'M',
  linux: 'L',
  esp32: 'E',
};

function getTypeAbbrev(deviceTypeName: string): string {
  return TYPE_ABBREVS[deviceTypeName.toLowerCase()] ?? '?';
}

// --- Graph types ---
export interface GraphNode {
  id: string;
  name: string;
  type: 'fleet' | 'device';
  val: number;
  color: string;
  deviceCount?: number;
  device?: Device;
  status?: string;
  deviceTypeName?: string;
  typeAbbrev?: string;
  // Cross-linked by buildForceGraphData after construction
  neighbors: GraphNode[];
  links: GraphLink[];
  // D3 adds these at runtime
  x?: number;
  y?: number;
}

export interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
}

export interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
}

export function buildForceGraphData(
  devices: Device[],
  fleets: Fleet[],
): GraphData {
  const nodes: GraphNode[] = [];
  const links: GraphLink[] = [];
  const nodeMap = new Map<string, GraphNode>();

  // Fleet hub nodes
  for (const fleet of fleets) {
    const node: GraphNode = {
      id: `fleet-${fleet.id}`,
      name: fleet.name,
      type: 'fleet',
      val: 30,
      color: FLEET_COLOR,
      deviceCount: fleet.device_count,
      neighbors: [],
      links: [],
    };
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  // Check for unassigned devices
  const hasUnassigned = devices.some((d) => d.fleet_id == null);
  if (hasUnassigned) {
    const node: GraphNode = {
      id: 'fleet-unassigned',
      name: 'Unassigned',
      type: 'fleet',
      val: 30,
      color: FLEET_COLOR,
      deviceCount: devices.filter((d) => d.fleet_id == null).length,
      neighbors: [],
      links: [],
    };
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  // Device nodes + links
  for (const device of devices) {
    const fleetNodeId =
      device.fleet_id != null ? `fleet-${device.fleet_id}` : 'fleet-unassigned';

    const node: GraphNode = {
      id: `device-${device.id}`,
      name: device.name,
      type: 'device',
      val: 4,
      color: STATUS_COLORS[device.status] ?? DEFAULT_COLOR,
      device,
      status: device.status,
      deviceTypeName: device.device_type_name,
      typeAbbrev: getTypeAbbrev(device.device_type_name),
      neighbors: [],
      links: [],
    };
    nodes.push(node);
    nodeMap.set(node.id, node);

    const link: GraphLink = { source: fleetNodeId, target: node.id };
    links.push(link);
  }

  // Cross-link neighbors (from the library's official highlight example)
  for (const link of links) {
    const a = nodeMap.get(link.source as string);
    const b = nodeMap.get(link.target as string);
    if (a && b) {
      a.neighbors.push(b);
      b.neighbors.push(a);
      a.links.push(link);
      b.links.push(link);
    }
  }

  return { nodes, links };
}
