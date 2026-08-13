import type { Device, DeviceType, Fleet } from '../../types/api';
import { STATUS_COLORS, FLEET_COLOR, DEFAULT_COLOR, TYPE_ABBREVS } from './constants';
import { getUptimeArcAngle, getHealthTier } from './health-utils';

function getTypeAbbrev(deviceTypeName: string): string {
  return TYPE_ABBREVS[deviceTypeName.toLowerCase()] ?? '?';
}

// --- Graph types ---
export interface GraphNode {
  id: string;
  name: string;
  type: 'fleet' | 'device' | 'external';
  val: number;
  color: string;
  deviceCount?: number;
  device?: Device;
  connection?: NonNullable<Device['declared_connections']>[number];
  /** Generic metadata rows used by other topology views reusing this canvas. */
  details?: Array<{ label: string; value: string }>;
  status?: string;
  deviceTypeName?: string;
  deviceTypeIcon?: string;
  deviceTypeColor?: string;
  typeAbbrev?: string;
  // Health data (computed from last_seen_at / uptime_seconds)
  lastSeenTimestamp?: number; // parsed epoch ms, cached for per-frame staleness
  uptimeSeconds?: number;
  uptimeArcAngle?: number; // radians, computed once per refresh
  // Fleet hub aggregate health (proportions 0-1)
  tierRatios?: { fresh: number; warm: number; stale: number; dead: number; never: number };
  // Cross-linked by buildForceGraphData after construction
  neighbors: GraphNode[];
  links: GraphLink[];
  // Stable force targets used to keep large fleets readable.
  layoutX?: number;
  layoutY?: number;
  layoutRadius?: number;
  // D3 adds these at runtime
  x?: number;
  y?: number;
}

export interface GraphLink {
  source: string | GraphNode;
  target: string | GraphNode;
  kind?: 'fleet' | 'declared';
  /** Optional force tuning for topology views with an intentional composition. */
  layoutDistance?: number;
  layoutStrength?: number;
  connection?: NonNullable<Device['declared_connections']>[number];
}

export interface GraphData {
  nodes: GraphNode[];
  links: GraphLink[];
}

const FLEET_SPACING = 620;
const STRUCTURED_LAYOUT_MIN_DEVICES = 11;
const DEVICE_RING_START_RADIUS = 120;
const DEVICE_RING_STEP = 58;
const DEVICE_RING_MIN_SPACING = 42;
const EXTERNAL_COLOR = '#7B8B9A';

const CONNECTION_TYPE_VISUALS: Array<[string[], { icon: string; color: string }]> = [
  [['router', 'gateway', 'network_gateway'], { icon: 'globe-network', color: '#8ABBFF' }],
  [['playstation', 'xbox', 'nintendo', 'console'], { icon: 'console', color: '#D982FF' }],
  [
    ['network_device', 'network_gear', 'tp_link', 'netgear', 'switch', 'access_point'],
    { icon: 'data-connection', color: '#36CFC9' },
  ],
  [['private_wifi', 'wifi', 'wi_fi', 'wireless'], { icon: 'cell-tower', color: '#36CFC9' }],
  [['apple_tv', 'appletv'], { icon: 'media', color: '#A7B0C0' }],
  [['iphone', 'ipad', 'watch'], { icon: 'mobile-phone', color: '#A7B0C0' }],
  [['mac', 'imac', 'macbook'], { icon: 'desktop', color: '#A7B0C0' }],
  [['apple_device'], { icon: 'desktop', color: '#A7B0C0' }],
  [
    ['android', 'samsung', 'google_device', 'xiaomi', 'huawei'],
    { icon: 'mobile-phone', color: '#7BD88F' },
  ],
  [['google_home', 'google_nest', 'nest'], { icon: 'home', color: '#7BD88F' }],
  [
    ['chromecast', 'roku', 'fire_tv', 'android_tv', 'smart_tv', 'tv', 'lg_device'],
    { icon: 'media', color: '#D982FF' },
  ],
  [['raspberry_pi', 'esp32', 'espressif', 'iot'], { icon: 'sim-card', color: '#F29D49' }],
  [['printer'], { icon: 'print', color: '#F7C948' }],
  [['nas', 'server', 'smb'], { icon: 'server', color: '#8ABBFF' }],
  [['camera', 'doorbell'], { icon: 'camera', color: '#E76A6E' }],
  [['speaker', 'sonos', 'spotify_connect'], { icon: 'volume-up', color: '#D982FF' }],
  [['hue', 'wemo', 'smart_home'], { icon: 'lightbulb', color: '#F7C948' }],
  [['sensor'], { icon: 'sensor', color: '#7BD88F' }],
  [['phone'], { icon: 'phone', color: '#7BD88F' }],
  [['vehicle'], { icon: 'known-vehicle', color: '#F29D49' }],
  [['host', 'ip'], { icon: 'ip-address', color: EXTERNAL_COLOR }],
];

function normalizeDeviceTypeKey(value?: string | null): string {
  return value?.trim().toLowerCase().replaceAll('-', '_') ?? '';
}

function getConnectionSearchText(
  connection: NonNullable<Device['declared_connections']>[number],
): string {
  return [
    connection.device_type,
    connection.connection_type,
    connection.label,
    connection.source,
    connection.external_id,
  ]
    .map(normalizeDeviceTypeKey)
    .filter(Boolean)
    .join(' ');
}

function getConnectionVisual(connection: NonNullable<Device['declared_connections']>[number]): {
  icon: string;
  color: string;
} {
  const searchText = getConnectionSearchText(connection);

  for (const [tokens, visual] of CONNECTION_TYPE_VISUALS) {
    if (tokens.some((token) => searchText.includes(token))) {
      return visual;
    }
  }

  return { icon: 'cube', color: EXTERNAL_COLOR };
}

function connectionNodeId(sourceDeviceId: string, connectionId: string): string {
  return `connection-${sourceDeviceId}-${encodeURIComponent(connectionId)}`;
}

function getConnectionLabel(connection: NonNullable<Device['declared_connections']>[number]) {
  return (
    connection.label ||
    connection.device_id ||
    connection.external_id ||
    connection.address ||
    'External'
  );
}

function getFleetAnchor(index: number, total: number): { x: number; y: number } {
  if (total <= 1) return { x: 0, y: 0 };

  const columns = Math.ceil(Math.sqrt(total));
  const rows = Math.ceil(total / columns);
  const col = index % columns;
  const row = Math.floor(index / columns);

  return {
    x: (col - (columns - 1) / 2) * FLEET_SPACING,
    y: (row - (rows - 1) / 2) * FLEET_SPACING,
  };
}

function getRingPosition(
  index: number,
  total: number,
  center: { x: number; y: number },
): { x: number; y: number; radius: number } {
  let remaining = index;
  let ring = 0;

  while (true) {
    const radius = DEVICE_RING_START_RADIUS + ring * DEVICE_RING_STEP;
    const capacity = Math.max(8, Math.floor((2 * Math.PI * radius) / DEVICE_RING_MIN_SPACING));

    if (remaining < capacity) {
      const devicesOnRing = Math.min(capacity, total - (index - remaining));
      const angleStep = (2 * Math.PI) / devicesOnRing;
      const angle = remaining * angleStep - Math.PI / 2 + (ring % 2 === 0 ? 0 : angleStep / 2);

      return {
        x: center.x + Math.cos(angle) * radius,
        y: center.y + Math.sin(angle) * radius,
        radius,
      };
    }

    remaining -= capacity;
    ring += 1;
  }
}

export function buildForceGraphData(
  devices: Device[],
  fleets: Fleet[],
  prevNodes?: GraphNode[],
  deviceTypes?: DeviceType[],
): GraphData {
  const nodes: GraphNode[] = [];
  const links: GraphLink[] = [];
  const nodeMap = new Map<string, GraphNode>();
  const deviceNodeIdByDeviceId = new Map<string, string>();
  const deviceTypeByName = new Map(
    (deviceTypes ?? []).map((deviceType) => [normalizeDeviceTypeKey(deviceType.name), deviceType]),
  );

  const prevNodeMap = new Map<string, GraphNode>();
  if (prevNodes) {
    for (const n of prevNodes) {
      prevNodeMap.set(n.id, n);
    }
  }

  const devicesByFleet = new Map<string, Device[]>();
  for (const device of devices) {
    const fleetNodeId = device.fleet_id != null ? `fleet-${device.fleet_id}` : 'fleet-unassigned';
    const groupedDevices = devicesByFleet.get(fleetNodeId);
    if (groupedDevices) {
      groupedDevices.push(device);
    } else {
      devicesByFleet.set(fleetNodeId, [device]);
    }
  }

  const fleetNodeIds = [
    ...fleets.map((fleet) => `fleet-${fleet.id}`),
    ...(devicesByFleet.has('fleet-unassigned') ? ['fleet-unassigned'] : []),
  ];
  const structuredFleetNodeIds = new Set(
    fleetNodeIds.filter(
      (nodeId) => (devicesByFleet.get(nodeId)?.length ?? 0) >= STRUCTURED_LAYOUT_MIN_DEVICES,
    ),
  );

  const fleetAnchors = new Map<string, { x: number; y: number }>();
  Array.from(structuredFleetNodeIds).forEach((nodeId, index) => {
    const prev = prevNodeMap.get(nodeId);
    const fallback = getFleetAnchor(index, structuredFleetNodeIds.size);
    fleetAnchors.set(nodeId, {
      x: prev?.layoutX ?? fallback.x,
      y: prev?.layoutY ?? fallback.y,
    });
  });

  // Fleet hub nodes
  for (const fleet of fleets) {
    const nodeId = `fleet-${fleet.id}`;
    const prev = prevNodeMap.get(nodeId);
    const anchor = fleetAnchors.get(nodeId);
    const usesStructuredLayout = structuredFleetNodeIds.has(nodeId) && anchor != null;
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
      layoutX: usesStructuredLayout ? anchor.x : undefined,
      layoutY: usesStructuredLayout ? anchor.y : undefined,
      layoutRadius: usesStructuredLayout ? 0 : undefined,
    };
    if (prev) {
      node.x = prev.x;
      node.y = prev.y;
    } else if (usesStructuredLayout) {
      node.x = anchor.x;
      node.y = anchor.y;
    }
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  // Check for unassigned devices
  const hasUnassigned = devices.some((d) => d.fleet_id == null);
  if (hasUnassigned) {
    const nodeId = 'fleet-unassigned';
    const prev = prevNodeMap.get(nodeId);
    const anchor = fleetAnchors.get(nodeId);
    const usesStructuredLayout = structuredFleetNodeIds.has(nodeId) && anchor != null;
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
      layoutX: usesStructuredLayout ? anchor.x : undefined,
      layoutY: usesStructuredLayout ? anchor.y : undefined,
      layoutRadius: usesStructuredLayout ? 0 : undefined,
    };
    if (prev) {
      node.x = prev.x;
      node.y = prev.y;
    } else if (usesStructuredLayout) {
      node.x = anchor.x;
      node.y = anchor.y;
    }
    nodes.push(node);
    nodeMap.set(node.id, node);
  }

  const deviceLayoutById = new Map<string, { x: number; y: number; radius: number }>();
  for (const [fleetNodeId, groupedDevices] of devicesByFleet) {
    if (!structuredFleetNodeIds.has(fleetNodeId)) continue;
    const anchor = fleetAnchors.get(fleetNodeId);
    if (!anchor) continue;

    groupedDevices.sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
    groupedDevices.forEach((device, index) => {
      deviceLayoutById.set(device.id, getRingPosition(index, groupedDevices.length, anchor));
    });
  }

  // Device nodes + links
  for (const device of devices) {
    const fleetNodeId = device.fleet_id != null ? `fleet-${device.fleet_id}` : 'fleet-unassigned';

    const nodeId = `device-${device.id}`;
    const prev = prevNodeMap.get(nodeId);
    const lastSeenTimestamp = device.last_seen_at ? new Date(device.last_seen_at).getTime() : NaN;
    const uptimeSeconds = device.uptime_seconds ?? 0;
    const layout = deviceLayoutById.get(device.id);

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
      deviceTypeIcon: device.device_type_icon,
      deviceTypeColor: device.device_type_color_hex,
      typeAbbrev: getTypeAbbrev(device.device_type_name),
      lastSeenTimestamp,
      uptimeSeconds,
      uptimeArcAngle: getUptimeArcAngle(uptimeSeconds),
      neighbors: [],
      links: [],
      layoutX: layout?.x,
      layoutY: layout?.y,
      layoutRadius: layout?.radius,
    };
    if (prev) {
      node.x = prev.x;
      node.y = prev.y;
    } else if (layout) {
      node.x = layout.x;
      node.y = layout.y;
    }
    nodes.push(node);
    nodeMap.set(node.id, node);
    deviceNodeIdByDeviceId.set(device.id, node.id);

    const link: GraphLink = { source: fleetNodeId, target: node.id, kind: 'fleet' };
    links.push(link);
  }

  const declaredLinkKeys = new Set<string>();
  for (const device of devices) {
    const sourceNodeId = deviceNodeIdByDeviceId.get(device.id);
    if (!sourceNodeId) continue;

    for (const connection of device.declared_connections ?? []) {
      let targetNodeId = connection.device_id
        ? deviceNodeIdByDeviceId.get(connection.device_id)
        : undefined;
      if (targetNodeId === sourceNodeId) continue;

      if (!targetNodeId) {
        const externalId =
          connection.id ||
          connection.external_id ||
          connection.address ||
          getConnectionLabel(connection);
        targetNodeId = connectionNodeId(device.id, externalId);

        if (!nodeMap.has(targetNodeId)) {
          const prev = prevNodeMap.get(targetNodeId);
          const sourceLayout = deviceLayoutById.get(device.id);
          const deviceTypeName = connection.device_type ?? connection.connection_type;
          const deviceType = deviceTypeByName.get(normalizeDeviceTypeKey(deviceTypeName));
          const visual = getConnectionVisual(connection);
          const node: GraphNode = {
            ...prev,
            id: targetNodeId,
            name: getConnectionLabel(connection),
            type: 'external',
            val: 2,
            color:
              STATUS_COLORS[connection.status ?? ''] ?? deviceType?.color_hex ?? EXTERNAL_COLOR,
            connection,
            status: connection.status ?? 'external',
            deviceTypeName,
            deviceTypeIcon: deviceType?.icon ?? visual.icon,
            deviceTypeColor: deviceType?.color_hex ?? visual.color,
            typeAbbrev: '?',
            neighbors: [],
            links: [],
            layoutX: sourceLayout ? sourceLayout.x + 72 : undefined,
            layoutY: sourceLayout ? sourceLayout.y + 72 : undefined,
            layoutRadius: sourceLayout ? sourceLayout.radius + 72 : undefined,
          };
          if (prev) {
            node.x = prev.x;
            node.y = prev.y;
          } else if (sourceLayout) {
            node.x = sourceLayout.x + 72;
            node.y = sourceLayout.y + 72;
          }
          nodes.push(node);
          nodeMap.set(node.id, node);
        }
      }

      const linkKey = `${sourceNodeId}->${targetNodeId}`;
      if (declaredLinkKeys.has(linkKey)) continue;
      declaredLinkKeys.add(linkKey);
      links.push({
        source: sourceNodeId,
        target: targetNodeId,
        kind: 'declared',
        connection,
      });
    }
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
      node.tierRatios = { fresh: 0, warm: 0, stale: 0, dead: 0, never: 0 };
      continue;
    }
    const now = Date.now();
    let fresh = 0,
      warm = 0,
      stale = 0,
      dead = 0,
      never = 0;
    for (const dn of deviceNeighbors) {
      const staleness = dn.lastSeenTimestamp ? now - dn.lastSeenTimestamp : NaN;
      const tier = getHealthTier(staleness, dn.status);
      if (tier === 'fresh') fresh++;
      else if (tier === 'warm') warm++;
      else if (tier === 'stale') stale++;
      else if (tier === 'never') never++;
      else dead++;
    }
    node.tierRatios = {
      fresh: fresh / total,
      warm: warm / total,
      stale: stale / total,
      dead: dead / total,
      never: never / total,
    };
  }

  return { nodes, links };
}
