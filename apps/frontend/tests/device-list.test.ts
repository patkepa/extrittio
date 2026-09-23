import assert from 'node:assert/strict';
import test from 'node:test';

import {
  countDeviceStatuses,
  filterAndSortDevices,
} from '../src/features/devices/model/device-list.ts';

const devices = [
  { name: 'Beta', status: 'offline', last_seen_at: '2026-08-01', uptime_seconds: 7200 },
  { name: 'Alpha', status: 'online', last_seen_at: '2026-08-03', uptime_seconds: 36000 },
  { name: 'Gamma', status: 'online', last_seen_at: null, uptime_seconds: 3600 },
];

test('filters devices before applying a stable domain sort', () => {
  const result = filterAndSortDevices(devices, 'online', 'name', 'asc');
  assert.deepEqual(
    result.map((device) => device.name),
    ['Alpha', 'Gamma'],
  );
});

test('sort direction is applied to timestamp fields', () => {
  const result = filterAndSortDevices(devices, 'all', 'last_seen', 'desc');
  assert.deepEqual(
    result.map((device) => device.name),
    ['Alpha', 'Beta', 'Gamma'],
  );
});

test('sorts uptime by duration instead of display text', () => {
  const result = filterAndSortDevices(devices, 'all', 'uptime', 'asc');
  assert.deepEqual(result.map((device) => device.name), ['Gamma', 'Beta', 'Alpha']);
});

test('counts only recognized online and offline states', () => {
  assert.deepEqual(countDeviceStatuses([...devices, { name: 'Warn', status: 'warning' }]), {
    all: 4,
    online: 2,
    offline: 1,
  });
});
