import assert from 'node:assert/strict';
import test from 'node:test';

import { buildThreadMeshGraphData } from '../src/pages/openthread/thread-mesh-graph-data.ts';
import type { ThreadMeshDevice, ThreadMeshScan, ThreadStatus } from '../src/types/api.ts';

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
  const scan: ThreadMeshScan = {
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
});
