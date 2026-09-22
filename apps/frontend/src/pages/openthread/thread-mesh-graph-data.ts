import type {
  GraphData,
  GraphLink,
  GraphNode,
} from '../../components/fleet-graph/build-force-graph-data';
import type {
  ThreadMeshDevice,
  ThreadNetworkDiagnostics,
  ThreadNetwork,
  ThreadStatus,
} from '../../types/api';

const CURRENT_NETWORK_COLOR = '#2D72D2';
const NEARBY_NETWORK_COLOR = '#5C7080';
const BORDER_ROUTER_COLOR = '#36CFC9';
const ROUTER_COLOR = '#8ABBFF';
const CHILD_COLOR = '#7BD88F';
const SLEEPY_CHILD_COLOR = '#D982FF';
const BORDER_ROUTER_X = -80;
const CURRENT_NETWORK_X = 120;
const NEARBY_NETWORK_X = -340;
const MESH_DEVICE_X = 360;

function normalizedHex(value?: string | null): string {
  return value?.trim().toLowerCase().replace(/^0x/, '').padStart(4, '0') ?? '';
}

export function isCurrentThreadNetwork(network: ThreadNetwork, status: ThreadStatus): boolean {
  const panMatches = normalizedHex(network.pan_id) === normalizedHex(status.pan_id);
  const nameMatches = Boolean(network.network_name) && network.network_name === status.network_name;
  return network.channel === status.channel && (panMatches || nameMatches);
}

function text(value: string | number | boolean | null | undefined): string | undefined {
  if (value == null || value === '') return undefined;
  if (typeof value === 'boolean') return value ? 'Yes' : 'No';
  return String(value);
}

function details(
  rows: Array<[string, string | number | boolean | null | undefined]>,
): Array<{ label: string; value: string }> {
  return rows.flatMap(([label, value]) => {
    const formatted = text(value);
    return formatted ? [{ label, value: formatted }] : [];
  });
}

function roleLabel(role?: string | null): string {
  if (!role) return 'Thread node';
  return role
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function deviceName(device: ThreadMeshDevice): string {
  if (device.is_border_router) return device.hostname || 'Extrittio Border Router';
  return device.hostname || `${roleLabel(device.role)} · ${device.id.slice(-6)}`;
}

function deviceVisual(device: ThreadMeshDevice): { color: string; icon: string } {
  if (device.is_border_router) return { color: BORDER_ROUTER_COLOR, icon: 'satellite' };
  if (device.role === 'router' || device.role === 'leader') {
    return { color: ROUTER_COLOR, icon: 'globe-network' };
  }
  if (device.rx_on_when_idle === false) {
    return { color: SLEEPY_CHILD_COLOR, icon: 'moon' };
  }
  return { color: CHILD_COLOR, icon: 'data-connection' };
}

function deviceDetails(device: ThreadMeshDevice) {
  return details([
    ['Node type', device.is_border_router ? 'Extrittio border router' : 'Thread client'],
    ['Role', roleLabel(device.role)],
    ['Hostname', device.hostname],
    ['Extended address', device.extended_address],
    ['EUI-64', device.eui64],
    ['RLOC16', device.rloc16],
    ['RLOC address', device.rloc_address],
    ['Mesh-local EID IID', device.mesh_local_eid_iid],
    ['OMR IPv6', device.omr_ipv6_addresses.join(', ')],
    ['Full Thread device', device.full_thread_device],
    ['Receiver always on', device.rx_on_when_idle],
    ['Full network data', device.full_network_data],
    ['Router ID', device.router_id],
    ['Network', device.network_name],
    ['Extended PAN ID', device.extended_pan_id],
    ['Known routers', device.router_count],
    ['Partition ID', device.partition_id],
    ['Leader router ID', device.leader_router_id],
    ['Data version', device.data_version],
    ['Stable data version', device.stable_data_version],
    ['Border Agent state', device.border_agent_state],
    ['Border Agent ID', device.border_agent_id],
    ['First discovered', device.created_at],
    ['Last updated', device.updated_at],
  ]);
}

function nearbyNetworkDetails(network: ThreadNetwork) {
  return details([
    ['Node type', 'Nearby Thread network'],
    ['Network name', network.network_name || 'Unnamed network'],
    ['PAN ID', network.pan_id],
    ['Beacon address', network.extended_address],
    ['Channel', network.channel],
    ['Signal', `${network.rssi} dBm`],
    ['Link quality', network.lqi],
  ]);
}

function currentNetworkDetails(status: ThreadStatus) {
  return details([
    ['Node type', 'Active Thread network'],
    ['Network name', status.network_name],
    ['Border-router role', roleLabel(status.role)],
    ['Channel', status.channel],
    ['PAN ID', status.pan_id],
    ['Extended PAN ID', status.extended_pan_id],
    ['Mesh-local prefix', status.mesh_local_prefix],
    ['Border-router addresses', status.addresses.join(', ')],
  ]);
}

function columnPosition(
  index: number,
  total: number,
  options: {
    startX: number;
    direction: -1 | 1;
    maxRows: number;
    rowSpacing: number;
    columnSpacing: number;
  },
) {
  const column = Math.floor(index / options.maxRows);
  const row = index % options.maxRows;
  const rowsInColumn = Math.min(options.maxRows, total - column * options.maxRows);

  return {
    x: options.startX + column * options.columnSpacing * options.direction,
    y: (row - (rowsInColumn - 1) / 2) * options.rowSpacing,
  };
}

function distanceBetween(
  source: Pick<GraphNode, 'layoutX' | 'layoutY'>,
  target: Pick<GraphNode, 'layoutX' | 'layoutY'>,
) {
  return Math.hypot(
    (target.layoutX ?? 0) - (source.layoutX ?? 0),
    (target.layoutY ?? 0) - (source.layoutY ?? 0),
  );
}

export function buildThreadMeshGraphData(
  scan: Pick<ThreadNetworkDiagnostics, 'networks' | 'devices'>,
  status: ThreadStatus,
  previousNodes?: GraphNode[],
  positionOverrides?: ReadonlyMap<string, { x: number; y: number }>,
): GraphData {
  const nodes: GraphNode[] = [];
  const links: GraphLink[] = [];
  const nodeMap = new Map<string, GraphNode>();
  const previousById = new Map((previousNodes ?? []).map((node) => [node.id, node]));
  const meshDevices = scan.devices.filter((device) => !device.is_border_router);
  const nearbyNetworks = scan.networks.filter(
    (network) => !isCurrentThreadNetwork(network, status),
  );
  const currentNetworkId = 'thread-network-current';

  const addNode = (node: GraphNode) => {
    const previous = previousById.get(node.id);
    const override = positionOverrides?.get(node.id);
    const merged =
      previous || override
        ? {
            ...node,
            x: override?.x ?? previous?.x,
            y: override?.y ?? previous?.y,
            layoutX: override?.x ?? previous?.layoutX ?? node.layoutX,
            layoutY: override?.y ?? previous?.layoutY ?? node.layoutY,
          }
        : node;
    nodes.push(merged);
    nodeMap.set(merged.id, merged);
    return merged;
  };

  const currentNetwork = addNode({
    id: currentNetworkId,
    name: status.network_name || 'Active Thread network',
    type: 'fleet',
    val: 32,
    color: CURRENT_NETWORK_COLOR,
    deviceCount: meshDevices.length,
    details: currentNetworkDetails(status),
    neighbors: [],
    links: [],
    layoutX: CURRENT_NETWORK_X,
    layoutY: 0,
    layoutRadius: 0,
    x: CURRENT_NETWORK_X,
    y: 0,
  });

  const discoveredBorderRouter = scan.devices.find((device) => device.is_border_router);
  const borderRouterId = discoveredBorderRouter
    ? `thread-device-${discoveredBorderRouter.id}`
    : 'thread-device-border-router';
  const borderRouter = addNode({
    id: borderRouterId,
    name: discoveredBorderRouter ? deviceName(discoveredBorderRouter) : 'Extrittio Border Router',
    type: 'external',
    val: 6,
    color: BORDER_ROUTER_COLOR,
    visualColor: BORDER_ROUTER_COLOR,
    visualIcon: 'satellite',
    visualName: 'Border router',
    initialViewportAnchor: true,
    details: discoveredBorderRouter
      ? deviceDetails(discoveredBorderRouter)
      : details([
          ['Node type', 'Extrittio border router'],
          ['Role', roleLabel(status.role)],
          ['Network', status.network_name],
          ['RCP device', status.rcp_device],
          ['Addresses', status.addresses.join(', ')],
        ]),
    neighbors: [],
    links: [],
    layoutX: BORDER_ROUTER_X,
    layoutY: 0,
    layoutRadius: CURRENT_NETWORK_X - BORDER_ROUTER_X,
    x: BORDER_ROUTER_X,
    y: 0,
  });
  links.push({
    source: borderRouterId,
    target: currentNetworkId,
    kind: 'declared',
    layoutDistance: distanceBetween(borderRouter, currentNetwork),
    layoutStrength: 0.14,
  });

  meshDevices.forEach((device, index) => {
    const visual = deviceVisual(device);
    const position = columnPosition(index, meshDevices.length, {
      startX: MESH_DEVICE_X,
      direction: 1,
      maxRows: 7,
      rowSpacing: 68,
      columnSpacing: 145,
    });
    const id = `thread-device-${device.id}`;
    const meshDevice = addNode({
      id,
      name: deviceName(device),
      type: 'external',
      val: 3,
      color: visual.color,
      visualColor: visual.color,
      visualIcon: visual.icon,
      visualName: roleLabel(device.role),
      details: deviceDetails(device),
      neighbors: [],
      links: [],
      layoutX: position.x,
      layoutY: position.y,
      layoutRadius: distanceBetween(currentNetwork, {
        layoutX: position.x,
        layoutY: position.y,
      }),
      x: position.x,
      y: position.y,
    });
    links.push({
      source: currentNetworkId,
      target: id,
      kind: 'declared',
      layoutDistance: distanceBetween(currentNetwork, meshDevice),
      layoutStrength: 0.09,
    });
  });

  nearbyNetworks.forEach((network, index) => {
    const position = columnPosition(index, nearbyNetworks.length, {
      startX: NEARBY_NETWORK_X,
      direction: -1,
      maxRows: 5,
      rowSpacing: 92,
      columnSpacing: 180,
    });
    const id = `thread-network-nearby-${network.extended_address}-${network.pan_id}-${network.channel}`;
    const nearbyNetwork = addNode({
      id,
      name: network.network_name || `Unnamed · ${network.pan_id}`,
      type: 'fleet',
      val: 24,
      color: NEARBY_NETWORK_COLOR,
      details: nearbyNetworkDetails(network),
      neighbors: [],
      links: [],
      layoutX: position.x,
      layoutY: position.y,
      layoutRadius: distanceBetween(borderRouter, {
        layoutX: position.x,
        layoutY: position.y,
      }),
      x: position.x,
      y: position.y,
    });
    links.push({
      source: borderRouterId,
      target: id,
      kind: 'declared',
      layoutDistance: distanceBetween(borderRouter, nearbyNetwork),
      layoutStrength: 0.08,
    });
  });

  for (const link of links) {
    const source = nodeMap.get(link.source as string);
    const target = nodeMap.get(link.target as string);
    if (!source || !target) continue;
    source.neighbors.push(target);
    target.neighbors.push(source);
    source.links.push(link);
    target.links.push(link);
  }

  return { nodes, links };
}
