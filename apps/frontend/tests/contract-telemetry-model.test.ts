import assert from 'node:assert/strict';
import test from 'node:test';

import { contractMetricDefinitions } from '../src/components/devices/contract-telemetry-model.ts';
import type { DeviceContract } from '../src/types/api.ts';

function contract(document: unknown): DeviceContract {
  return {
    id: 'contract-1',
    device_id: 'device-1',
    blueprint_revision_id: 'revision-1',
    contract_hash: 'a'.repeat(64),
    assignment_status: 'converged',
    acknowledged_at: null,
    error: null,
    created_at: '2026-08-30T00:00:00Z',
    document,
  };
}

test('derives metric labels, types, units, presentation, and identity from the contract', () => {
  const definitions = contractMetricDefinitions(
    contract({
      streams: {
        environment: {
          fields: {
            '/humidity': {
              valueType: 'float64',
              label: 'Relative humidity',
              unit: '%',
            },
            '/temperature': {
              valueType: 'float64',
              label: 'Temperature',
              unit: '°C',
              presentation: { color: '#123456', chart: 'line', precision: 1 },
            },
          },
        },
      },
      presentation: {
        summary: [{ metric: 'environment./temperature' }],
      },
    }),
  );

  assert.deepEqual(
    definitions.map(({ key }) => key),
    ['environment./temperature', 'environment./humidity'],
  );
  assert.deepEqual(definitions[0], {
    key: 'environment./temperature',
    streamKey: 'environment',
    fieldPath: '/temperature',
    valueType: 'float64',
    label: 'Temperature',
    unit: '°C',
    color: '#123456',
    presentation: { color: '#123456', chart: 'line', precision: 1 },
  });
  assert.notEqual(definitions[1]?.color, definitions[0]?.color);
});

test('treats a contract without streams as an empty metric catalog', () => {
  assert.deepEqual(contractMetricDefinitions(contract({})), []);
});
