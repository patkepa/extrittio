import assert from 'node:assert/strict';
import test from 'node:test';
import { locationObservations } from '../src/components/devices/contract-location-model.ts';
import type { DeviceContract, DeviceMetric } from '../src/types/api.ts';

const contract: DeviceContract = {
  id: 'contract-1',
  device_id: 'device-1',
  blueprint_revision_id: 'revision-1',
  contract_hash: 'a'.repeat(64),
  assignment_status: 'converged',
  acknowledged_at: null,
  error: null,
  created_at: '2026-09-15T00:00:00Z',
  document: {
    location: {
      stream: 'tracking',
      latitudePath: '/north',
      longitudePath: '/east',
      maxAgeMs: 30000,
      coordinateSystem: 'wgs84',
      unit: 'degrees',
    },
  },
};
const metric = (
  event_id: string,
  field_path: string,
  value: number,
  overrides: Partial<DeviceMetric> = {},
): DeviceMetric => ({
  event_id,
  field_path,
  value,
  contract_id: contract.id,
  device_id: contract.device_id,
  occurred_at: '2026-09-15T00:00:00Z',
  stream_key: 'tracking',
  value_type: 'float64',
  ...overrides,
});

test('pairs coordinates only within one event and preserves origin', () => {
  assert.deepEqual(
    locationObservations(contract, [metric('a', '/north', 0), metric('b', '/east', 0)]),
    [],
  );
  const observations = locationObservations(contract, [
    metric('a', '/north', 0),
    metric('a', '/east', 0),
  ]);
  assert.equal(observations.length, 1);
  assert.equal(observations[0].latitude, 0);
  assert.equal(observations[0].longitude, 0);
});

test('rejects cross-contract, cross-stream, invalid and inconsistent coordinates', () => {
  for (const overrides of [
    { contract_id: 'old-contract' },
    { stream_key: 'other' },
    { device_id: 'other' },
    { occurred_at: '2026-09-15T00:00:01Z' },
    { value: Number.NaN },
    { value: 181 },
  ]) {
    assert.deepEqual(
      locationObservations(contract, [
        metric('a', '/north', 1),
        metric('a', '/east', 2, overrides),
      ]),
      [],
    );
  }
  assert.deepEqual(
    locationObservations(contract, [
      metric('a', '/north', 1),
      metric('a', '/north', 2),
      metric('a', '/east', 3),
    ]),
    [],
  );
  assert.deepEqual(
    locationObservations({ ...contract, document: {} }, [
      metric('a', '/north', 1),
      metric('a', '/east', 2),
    ]),
    [],
  );
});
