import assert from 'node:assert/strict';
import test from 'node:test';

import { buildThreadMeshGraphData } from '../src/pages/openthread/thread-mesh-graph-data.ts';
import type { ThreadMeshDevice, ThreadNetworkDiagnostics, ThreadStatus } from '../src/types/api.ts';

const status: ThreadStatus = {
  available: true,
  connected: true,
  error: null,
  rcp_device: '/dev/ttyACM0',
  available_rcp_devices: ['/dev/ttyACM0'],
  role: 'leader',
  network_name: 'Extrittio-Thread',
  channel: 15,
  pan_id: '1234',
  extended_pan_id: '0011223344556677',
  mesh_local_prefix: 'fd35:3441:33d1:d73e::/64',
  addresses: ['fd35:3441:33d1:d73e::1'],
};

function meshDevice(overrides: Partial<ThreadMeshDevice>): ThreadMeshDevice {
  return {
    id: '2a55d952bc7b4008',
    is_border_router: false,
    extended_address: '2a55d952bc7b4008',
    mesh_local_eid_iid: '2a55d952bc7b4008',
    omr_ipv6_addresses: ['fd11:22::2'],
    hostname: 'sensor-1',
    eui64: null,
    role: 'child',
    full_thread_device: false,
    rx_on_when_idle: false,
    full_network_data: true,
    rloc16: '0x5004',
    rloc_address: null,
    router_id: null,
    router_count: null,
    network_name: null,
    extended_pan_id: null,
    border_agent_id: null,
    border_agent_state: null,
    partition_id: null,
    leader_router_id: null,
    data_version: null,
    stable_data_version: null,
    created_at: null,
    updated_at: null,
    ...overrides,
  };
}

test('builds active mesh clients and nearby networks into one reusable graph', () => {
  const scan: ThreadNetworkDiagnostics = {
    scanning: false,
    scanned_at: '2026-08-13T12:00:00.000Z',
    error: null,
    channels: [],
    networks: [
      {
        network_name: 'Extrittio-Thread',
        pan_id: '1234',
        extended_address: '0011223344556677',
        channel: 15,
        rssi: -18,
        lqi: 3,
      },
      {
        network_name: 'Neighbor',
        pan_id: 'abcd',
        extended_address: 'ffeeddccbbaa0099',
        channel: 20,
        rssi: -62,
        lqi: 2,
      },
    ],
    devices: [
      meshDevice({
        id: '96518e5497d5b9f3',
        is_border_router: true,
        hostname: 'extrittio.local',
        role: 'leader',
      }),
      meshDevice({}),
    ],
    statistics: {
      cca_failure_rate_percent: null,
      latest_rssi_dbm: null,
      monitor_sample_count: null,
      tx_total: null,
      rx_total: null,
      tx_retries: null,
      tx_errors: null,
      rx_errors: null,
    },
    warnings: [],
  };

  const graph = buildThreadMeshGraphData(scan, status);

  assert.equal(graph.nodes.length, 4);
  assert.equal(graph.links.length, 3);
  assert.equal(graph.nodes.filter((node) => node.type === 'fleet').length, 2);
  assert.equal(graph.nodes.find((node) => node.id === 'thread-network-current')?.deviceCount, 1);
  assert.equal(
    graph.nodes
      .find((node) => node.name === 'Neighbor')
      ?.details?.find((row) => row.label === 'Signal')?.value,
    '-62 dBm',
  );
  assert.equal(
    graph.nodes
      .find((node) => node.name === 'sensor-1')
      ?.details?.find((row) => row.label === 'Role')?.value,
    'Child',
  );

  const borderRouter = graph.nodes.find((node) => node.name === 'extrittio.local');
  const currentNetwork = graph.nodes.find((node) => node.id === 'thread-network-current');
  const nearbyNetwork = graph.nodes.find((node) => node.name === 'Neighbor');
  const client = graph.nodes.find((node) => node.name === 'sensor-1');
  assert.ok(borderRouter?.layoutX != null);
  assert.ok(currentNetwork?.layoutX != null);
  assert.ok(nearbyNetwork?.layoutX != null);
  assert.ok(client?.layoutX != null);
  assert.ok(nearbyNetwork.layoutX < borderRouter.layoutX);
  assert.ok(borderRouter.layoutX < currentNetwork.layoutX);
  assert.ok(currentNetwork.layoutX < client.layoutX);
});

test('builds the current mesh before the first scan completes', () => {
  const graph = buildThreadMeshGraphData({ networks: [], devices: [] }, status);

  assert.equal(graph.nodes.length, 2);
  assert.equal(graph.links.length, 1);
  assert.equal(graph.nodes[0]?.name, 'Extrittio-Thread');
  assert.equal(graph.nodes[1]?.name, 'Extrittio Border Router');
});

test('keeps dense topology columns vertically compact', () => {
  const devices = Array.from({ length: 21 }, (_, index) =>
    meshDevice({ id: `device-${index}`, hostname: `sensor-${index}` }),
  );
  const networks = Array.from({ length: 11 }, (_, index) => ({
    network_name: `Neighbor ${index}`,
    pan_id: `a${index.toString().padStart(3, '0')}`,
    extended_address: `network-${index}`,
    channel: 11 + index,
    rssi: -50 - index,
    lqi: 2,
  }));

  const graph = buildThreadMeshGraphData({ networks, devices }, status);
  const peripheralNodes = graph.nodes.filter(
    (node) => node.id !== 'thread-network-current' && node.type !== 'external',
  );

  assert.ok(graph.nodes.every((node) => Math.abs(node.layoutY ?? 0) <= 204));
  assert.ok(peripheralNodes.every((node) => node.layoutX !== 0));
});

test('preserves a dragged node as its new soft layout target across scan refreshes', () => {
  const refreshedGraph = buildThreadMeshGraphData(
    { networks: [], devices: [meshDevice({ id: 'dragged-device' })] },
    status,
    undefined,
    new Map([['thread-device-dragged-device', { x: 512, y: 96 }]]),
  );
  const refreshed = refreshedGraph.nodes.find((node) => node.id === 'thread-device-dragged-device');

  assert.equal(refreshed?.x, 512);
  assert.equal(refreshed?.y, 96);
  assert.equal(refreshed?.layoutX, 512);
  assert.equal(refreshed?.layoutY, 96);
});
