import type { Device, Fleet } from '../../types/api';
import {
  STATUS_COLORS,
  FLEET_COLOR,
  DEFAULT_COLOR,
  TYPE_ABBREVS,
} from './constants';
import { getUptimeArcAngle, getHealthTier } from './health-utils';

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
  // Health data (computed from last_seen_at / uptime_seconds)
  lastSeenTimestamp?: number; // parsed epoch ms, cached for per-frame staleness
  uptimeSeconds?: number;
  uptimeArcAngle?: number;   // radians, computed once per refresh
  // Fleet hub aggregate health (proportions 0-1)
  tierRatios?: { fresh: number; warm: number; stale: number; dead: number };
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
  prevNodes?: GraphNode[],
): GraphData {
  const nodes: GraphNode[] = [];
  const links: GraphLink[] = [];
  const nodeMap = new Map<string, GraphNode>();

  const prevNodeMap = new Map<string, GraphNode>();
  if (prevNodes) {
    for (const n of prevNodes) {
      prevNodeMap.set(n.id, n);
    }
  }

  // Fleet hub nodes
  for (const fleet of fleets) {
    const nodeId = `fleet-${fleet.id}`;
    const prev = prevNodeMap.get(nodeId);
    const node: GraphNode = {
      ...prev,
      id: nodeId,
      name: fleet.name,
      type: 'fleet',
      val: 30,
      color: FLEET_COLOR,
      deviceCount: fleet.device_count,
      neighbors: [],
      links: [],
    };
    if (prev) { node.x = prev.x; node.y = prev.y; }
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  // Check for unassigned devices
  const hasUnassigned = devices.some((d) => d.fleet_id == null);
  if (hasUnassigned) {
    const nodeId = 'fleet-unassigned';
    const prev = prevNodeMap.get(nodeId);
    const node: GraphNode = {
      ...prev,
      id: nodeId,
      name: 'Unassigned',
      type: 'fleet',
      val: 30,
      color: FLEET_COLOR,
      deviceCount: devices.filter((d) => d.fleet_id == null).length,
      neighbors: [],
      links: [],
    };
    if (prev) { node.x = prev.x; node.y = prev.y; }
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  // Device nodes + links
  for (const device of devices) {
    const fleetNodeId =
      device.fleet_id != null ? `fleet-${device.fleet_id}` : 'fleet-unassigned';

    const nodeId = `device-${device.id}`;
    const prev = prevNodeMap.get(nodeId);
    const lastSeenTimestamp = device.last_seen_at
      ? new Date(device.last_seen_at).getTime()
      : NaN;
    const uptimeSeconds = device.uptime_seconds ?? 0;

    const node: GraphNode = {
      ...prev,
      id: nodeId,
      name: device.name,
      type: 'device',
      val: 4,
      color: STATUS_COLORS[device.status] ?? DEFAULT_COLOR,
      device,
      status: device.status,
      deviceTypeName: device.device_type_name,
      typeAbbrev: getTypeAbbrev(device.device_type_name),
      lastSeenTimestamp,
      uptimeSeconds,
      uptimeArcAngle: getUptimeArcAngle(uptimeSeconds),
      neighbors: [],
      links: [],
    };
    if (prev) { node.x = prev.x; node.y = prev.y; }
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

  // Compute fleet hub aggregate health ratios
  for (const node of nodes) {
    if (node.type !== 'fleet') continue;
    const deviceNeighbors = node.neighbors.filter((n) => n.type === 'device');
    const total = deviceNeighbors.length;
    if (total === 0) {
      node.tierRatios = { fresh: 0, warm: 0, stale: 0, dead: 0 };
      continue;
    }
    const now = Date.now();
    let fresh = 0, warm = 0, stale = 0, dead = 0;
    for (const dn of deviceNeighbors) {
      const staleness = dn.lastSeenTimestamp ? now - dn.lastSeenTimestamp : NaN;
      const tier = getHealthTier(staleness);
      if (tier === 'fresh') fresh++;
      else if (tier === 'warm') warm++;
      else if (tier === 'stale') stale++;
      else dead++;
    }
    node.tierRatios = {
      fresh: fresh / total,
      warm: warm / total,
      stale: stale / total,
      dead: dead / total,
    };
  }

  return { nodes, links };
}
